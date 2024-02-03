extern crate simplelog;
use anyhow::Result;
use chrono::{DateTime, Utc};
use protobuf::Message;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::cmp::Ordering;
use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

use crate::gps_processor::{self, ProcessResult};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};
use crate::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use crate::protos;

/* The main database, we are likely to store a lot of protobuf bytes in it,
less relational stuff. Basically we will use it as a file system with better
transaction support.

`ongoing_journey` contains structured gps data for the current ongoing journey.
Note that it contains detailed timestamp, but these timestamp will be removed
when finalizing the journey.

`journey` keeps all finalized journeys. It stores most data as raw protobuf
bytes and some index for faster lookup. Instead of storing a signle blob, it has
two parts: header and data, so most common operation only need to fetch and
deserialize the header.
*/

// 3 is the zstd default
pub const ZSTD_COMPRESS_LEVEL: i32 = 3;

#[allow(clippy::type_complexity)]
fn open_db_and_run_migration(
    support_dir: &str,
    file_name: &str,
    migrations: &Vec<&dyn Fn(&Transaction) -> Result<()>>,
) -> Result<Connection> {
    debug!("open and run migration for {}", file_name);
    let mut conn = rusqlite::Connection::open(Path::new(support_dir).join(file_name))?;
    let tx = conn.transaction()?;
    let create_db_metadata_sql = "
    CREATE TABLE IF NOT EXISTS `db_metadata` (
	`key`	TEXT NOT NULL,
	`value`	TEXT,
	PRIMARY KEY(`key`)
    )";
    tx.execute(create_db_metadata_sql, ())?;
    let version_str: Option<String> = tx
        .query_row(
            "SELECT `value` FROM `db_metadata` WHERE key='version'",
            [],
            |row| row.get(0),
        )
        .optional()?;

    let version = match version_str {
        None => 0,
        Some(s) => s.parse()?,
    };

    let target_version = migrations.len();
    debug!(
        "current version = {}, target_version = {}",
        version, target_version
    );
    match version.cmp(&target_version) {
        Ordering::Equal => (),
        Ordering::Less => {
            for i in (version)..target_version {
                info!("running migration for version: {}", i + 1);
                let f = migrations.get(i).unwrap();
                f(&tx)?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO `db_metadata` (key, value) VALUES (?1, ?2)",
                ("version", target_version.to_string()),
            )?;
        }
        Ordering::Greater => {
            bail!(
                "version too high: current version = {}, target_version = {}",
                version,
                target_version
            );
        }
    }
    tx.commit()?;
    Ok(conn)
}

pub struct Txn<'a> {
    db_txn: rusqlite::Transaction<'a>,
}

impl Txn<'_> {
    pub fn get_ongoing_journey(&self) -> Result<Option<OngoingJourney>> {
        // `id` in `ongoing_journey` is auto incremented.
        let mut query = self.db_txn.prepare(
            "SELECT timestamp_sec, lat, lng, process_result FROM ongoing_journey ORDER BY id;",
        )?;
        let results = query.query_map((), |row| {
            let timestamp_sec: i64 = row.get(0)?;
            let process_result: i8 = row.get(3)?;
            Ok((
                timestamp_sec,
                TrackPoint {
                    latitude: row.get(1)?,
                    longitude: row.get(2)?,
                },
                process_result,
            ))
        })?;

        let mut segmants = Vec::new();
        let mut current_segment = Vec::new();

        let mut start_timestamp_sec = None;
        let mut end_timestamp_sec = None;
        for result in results {
            let (timestamp_sec, track_point, process_result) = result?;
            end_timestamp_sec = Some(timestamp_sec);
            if start_timestamp_sec.is_none() {
                start_timestamp_sec = Some(timestamp_sec);
            }
            let need_break = process_result == ProcessResult::NewSegment.to_int();
            if need_break && !current_segment.is_empty() {
                segmants.push(TrackSegment {
                    track_points: current_segment,
                });
                current_segment = Vec::new();
            }
            current_segment.push(track_point);
        }
        if !current_segment.is_empty() {
            segmants.push(TrackSegment {
                track_points: current_segment,
            });
        }

        if segmants.is_empty() {
            Ok(None)
        } else {
            // must be `Some`
            let start = DateTime::from_timestamp(start_timestamp_sec.unwrap(), 0).unwrap();
            let end = DateTime::from_timestamp(end_timestamp_sec.unwrap(), 0).unwrap();
            Ok(Some(OngoingJourney {
                start,
                end,
                journey_vector: JourneyVector {
                    track_segments: segmants,
                },
            }))
        }
    }

    pub fn clear_journeys(&self) -> Result<()> {
        self.db_txn.execute("DELETE FROM journey;", ())?;
        Ok(())
    }

    pub fn insert_journey(&self, header: JourneyHeader, data: JourneyData) -> Result<()> {
        let journey_type = header.journey_type;
        if journey_type != data.type_() {
            bail!("[insert_journey] Mismatch journey type")
        }
        let end_timestamp_sec = header.end.timestamp();
        let id = header.id.clone();

        let header_bytes = header.to_proto().write_to_bytes()?;
        let mut data_bytes = Vec::new();
        data.serialize(&mut data_bytes)?;

        let sql = "INSERT INTO journey (id, end_timestamp_sec, type, header, data) VALUES (?1, ?2, ?3, ?4, ?5);";
        self.db_txn.execute(
            sql,
            (
                &id,
                end_timestamp_sec,
                journey_type.to_int(),
                header_bytes,
                data_bytes,
            ),
        )?;
        Ok(())
    }

    pub fn create_and_insert_journey(
        &self,
        start: Option<DateTime<Utc>>,
        end: DateTime<Utc>,
        created_at: Option<DateTime<Utc>>,
        journey_kind: JourneyKind,
        note: Option<String>,
        journey_data: JourneyData,
    ) -> Result<()> {
        let journey_type = journey_data.type_();
        // create new journey
        let header = JourneyHeader {
            id: Uuid::new_v4().as_hyphenated().to_string(),
            // we use id + revision as the equality check, revision can be any
            // string (e.g. uuid) but a short random should be good enough.
            revision: random_string::generate(8, random_string::charsets::ALPHANUMERIC),
            created_at: created_at.unwrap_or(Utc::now()),
            updated_at: None,
            end,
            start,
            journey_type,
            journey_kind,
            note,
        };
        self.insert_journey(header, journey_data)
    }

    pub fn finalize_ongoing_journey(&self) -> Result<()> {
        match self.get_ongoing_journey()? {
            None => (),
            Some(OngoingJourney {
                start,
                end,
                journey_vector,
            }) => {
                // TODO: we could have some additional post-processing of the track.
                // including path refinement + lossy compression.

                // TODO: allow user to set this when recording?
                let journey_kind = JourneyKind::DefaultKind;

                self.create_and_insert_journey(
                    Some(start),
                    end,
                    None,
                    journey_kind,
                    None,
                    JourneyData::Vector(journey_vector),
                )?;
            }
        }

        self.db_txn.execute("DELETE FROM ongoing_journey;", ())?;

        Ok(())
    }

    pub fn list_all_journeys(&self) -> Result<Vec<JourneyHeader>> {
        let mut query = self.db_txn.prepare(
            "SELECT header, type FROM journey ORDER BY end_timestamp_sec, id DESC;",
            // use `id` to break tie
        )?;
        let mut rows = query.query(())?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let header_bytes = row.get_ref(0)?.as_blob()?;
            let journey_type = JourneyType::of_int(row.get(1)?)?;
            let header =
                JourneyHeader::of_proto(protos::journey::Header::parse_from_bytes(header_bytes)?)?;
            if header.journey_type != journey_type {
                bail!(
                    "Invalid DB state, `journey_type` miss match. id: {}.",
                    header.id
                );
            }
            results.push(header);
        }
        Ok(results)
    }

    pub fn get_journey(&self, id: &str) -> Result<JourneyData> {
        let mut query = self
            .db_txn
            .prepare("SELECT type, data FROM journey WHERE id = ?1;")?;

        query.query_row([id], |row| {
            let type_ = row.get_ref(0)?.as_i64()?;
            let f = || {
                let journey_type = JourneyType::of_int(i8::try_from(type_)?)?;
                let data = row.get_ref(1)?.as_blob()?;
                JourneyData::deserialize(data, journey_type)
            };
            Ok(f())
        })?
    }
}

pub struct MainDb {
    conn: Connection,
}

pub struct OngoingJourney {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub journey_vector: JourneyVector,
}

impl MainDb {
    pub fn open(support_dir: &str) -> MainDb {
        // TODO: better error handling
        let conn = open_db_and_run_migration(
            support_dir,
            "main.db",
            &vec![&|tx| {
                let sql = "
                CREATE TABLE ongoing_journey (
                    id             INTEGER PRIMARY KEY AUTOINCREMENT
                                        UNIQUE
                                        NOT NULL,
                    timestamp_sec  INTEGER NOT NULL,
                    lat            REAL    NOT NULL,
                    lng            REAL    NOT NULL,
                    process_result INTEGER NOT NULL
                );
                CREATE TABLE journey (
                    id                TEXT    PRIMARY KEY
                                              NOT NULL
                                              UNIQUE,
                    end_timestamp_sec INTEGER NOT NULL,
                    type              INTEGER NOT NULL,
                    header            BLOB    NOT NULL,
                    data              BLOB    NOT NULL
                );
                CREATE INDEX end_time_index ON journey (
                    end_timestamp_sec DESC
                );
                CREATE TABLE setting (
                    key               TEXT    PRIMARY KEY
                                              NOT NULL
                                              UNIQUE,
                    value             TEXT
                );
                ";
                for s in sql_split::split(sql) {
                    tx.execute(&s, ())?;
                }
                Ok(())
            }],
        )
        .expect("failed to open main db");
        MainDb { conn }
    }

    pub fn with_txn<F, O>(&mut self, f: F) -> Result<O>
    where
        F: FnOnce(&Txn) -> Result<O>,
    {
        let txn = Txn {
            db_txn: self.conn.transaction()?,
        };
        let output = f(&txn)?;
        txn.db_txn.commit()?;
        Ok(output)
    }

    pub fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }

    fn append_ongoing_journey(
        &mut self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
    ) -> Result<()> {
        let process_result = process_result.to_int();
        assert!(process_result >= 0);
        let tx = self.conn.transaction()?;
        let sql = "INSERT INTO ongoing_journey (timestamp_sec, lat, lng, process_result) VALUES (?1, ?2, ?3, ?4);";
        tx.prepare_cached(sql)?.execute((
            raw_data.timestamp_ms / 1000,
            raw_data.latitude,
            raw_data.longitude,
            process_result,
        ))?;
        tx.commit()?;
        Ok(())
    }

    pub fn record(
        &mut self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
    ) -> Result<()> {
        match process_result {
            ProcessResult::Ignore => (),
            ProcessResult::Append | ProcessResult::NewSegment => {
                self.append_ongoing_journey(raw_data, process_result)?;
            }
        }
        Ok(())
    }

    fn get_setting<T: FromStr>(&mut self, setting: Setting) -> Result<Option<T>>
    where
        <T as FromStr>::Err: Error + Send + Sync + 'static,
    {
        let tx = self.conn.transaction()?;
        let mut query = tx.prepare("SELECT value FROM setting WHERE key = ?1;")?;
        let result: Option<String> = query
            .query_row([setting.to_db_key()], |row| row.get(0))
            .optional()?;
        match result {
            None => Ok(None),
            Some(s) => {
                let v = FromStr::from_str(&s)?;
                Ok(Some(v))
            }
        }
    }

    pub fn get_setting_with_default<T: FromStr>(&mut self, setting: Setting, default: T) -> T
    where
        <T as FromStr>::Err: Error + Send + Sync + 'static,
    {
        match self.get_setting(setting) {
            Ok(v) => v,
            Err(error) => {
                warn!(
                    "[main_db.get_setting_with_default] setting:{:?}, error:{}",
                    setting, error
                );
                None
            }
        }
        .unwrap_or(default)
    }

    pub fn set_setting<T: ToString>(&mut self, setting: Setting, value: T) -> Result<()> {
        let tx = self.conn.transaction()?;
        let sql = "INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2);";
        tx.execute(sql, (setting.to_db_key(), value.to_string()))?;
        tx.commit()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Setting {
    RawDataMode,
}

impl Setting {
    fn to_db_key(self) -> &'static str {
        match self {
            Self::RawDataMode => "RAW_DATA_MODE",
        }
    }
}
