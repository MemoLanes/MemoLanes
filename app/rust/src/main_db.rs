extern crate simplelog;
use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate, Utc};
use protobuf::Message;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::cmp::Ordering;
use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

use crate::gps_processor::{self, GpsPostprocessor, PreprocessedData, ProcessResult};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};
use crate::journey_vector::{JourneyVector, TrackPoint};
use crate::{protos, utils};

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
    migrations: &[&dyn Fn(&Transaction) -> Result<()>],
) -> Result<Connection> {
    debug!("open and run migration for {}", file_name);
    let mut conn = rusqlite::Connection::open(Path::new(support_dir).join(file_name))?;
    let tx = conn.transaction()?;

    let version = utils::db::init_metadata_and_get_version(&tx)? as usize;
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
            utils::db::set_version_in_metadata(&tx, target_version as i32)?;
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
    pub action: Option<Action>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    Merge { journey_ids: Vec<String> },
    CompleteRebuilt,
}

fn generate_random_revision() -> String {
    random_string::generate(8, random_string::charsets::ALPHANUMERIC)
}

// NOTE: the `Txn` here is not only for making operation atomic, the `storage`
// will also use this to make sure the `cache_db` is in sync.
impl Txn<'_> {
    pub fn get_ongoing_journey(&self) -> Result<Option<OngoingJourney>> {
        // `id` in `ongoing_journey` is auto incremented.
        let mut query = self.db_txn.prepare(
            "SELECT timestamp_sec, lat, lng, process_result FROM ongoing_journey ORDER BY id;",
        )?;
        let results = query.query_map((), |row| {
            let timestamp_sec: Option<i64> = row.get(0)?;
            let process_result: i8 = row.get(3)?;
            Ok(PreprocessedData {
                timestamp_sec,
                track_point: TrackPoint {
                    latitude: row.get(1)?,
                    longitude: row.get(2)?,
                },
                process_result: process_result.into(),
            })
        })?;
        gps_processor::build_vector_journey(results.map(|x| x.map_err(|x| x.into())))
    }

    // the fist timestamp is the start time, the second is the end time
    pub fn get_ongoing_journey_timestamp_range(
        &self,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>> {
        // `id` in `ongoing_journey` is auto incremented and I assume it has index, so I didn't just linear scan timestamp.
        let mut query = self
            .db_txn
            .prepare("SELECT * FROM (SELECT timestamp_sec FROM ongoing_journey ORDER BY id ASC LIMIT 1) UNION ALL SELECT * FROM (SELECT timestamp_sec FROM ongoing_journey ORDER BY id DESC LIMIT 1);")?;
        let mut results = query.query_map((), |row| {
            // `timestamp_sec` cannot be null
            let timestamp_sec: i64 = row.get(0)?;
            Ok(timestamp_sec)
        })?;

        match results.next() {
            None => Ok(None),
            Some(start_timestamp_sec) => {
                let end_timestamp_sec = results.next().unwrap(); // must have
                let start = DateTime::from_timestamp(start_timestamp_sec?, 0).unwrap();
                let end = DateTime::from_timestamp(end_timestamp_sec?, 0).unwrap();
                Ok(Some((start, end)))
            }
        }
    }

    pub fn delete_all_journeys(&mut self) -> Result<()> {
        info!("Deleting all journeys");
        self.db_txn.execute("DELETE FROM journey;", ())?;
        self.action = Some(Action::CompleteRebuilt);
        Ok(())
    }

    pub fn delete_journey(&mut self, id: &str) -> Result<()> {
        info!("Deleting journey: id={}", id);
        let changes = self
            .db_txn
            .execute("DELETE FROM journey WHERE id = ?1;", (id,))?;
        self.action = Some(Action::CompleteRebuilt);
        if changes == 1 {
            Ok(())
        } else {
            Err(anyhow!("Failed to find journey with id = {}", id))
        }
    }

    // TODO: consider return structured result so the caller know if it is skipped or other cases
    pub fn insert_journey(&mut self, header: JourneyHeader, data: JourneyData) -> Result<()> {
        let journey_type = header.journey_type;
        if journey_type != data.type_() {
            bail!("[insert_journey] Mismatch journey type")
        }
        let id = header.id.clone();

        match self.get_journey_header(&id)? {
            Some(existing_header) => {
                if existing_header.revision == header.revision {
                    info!(
                        "Journey with ID {} already exists with the same revision, skip insert",
                        &header.id
                    );
                    return Ok(());
                } else {
                    bail!(
                        "Journey with ID {} already exists but with a different revision",
                        &header.id
                    );
                }
            }
            None => {
                info!(
                    "No existing journey found for id={}, proceed to insert new journey",
                    id
                );
            }
        }

        let journey_date = utils::date_to_days_since_epoch(header.journey_date);
        // use start time first, then fallback to endtime
        let timestamp_for_ordering = header.start.or(header.end).map(|x| x.timestamp());

        let header_bytes = header.to_proto().write_to_bytes()?;
        let mut data_bytes = Vec::new();
        data.serialize(&mut data_bytes)?;

        let sql = "INSERT INTO journey (id, journey_date, timestamp_for_ordering, type, header, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6);";
        self.db_txn.execute(
            sql,
            (
                &id,
                journey_date,
                timestamp_for_ordering,
                journey_type.to_int(),
                header_bytes,
                data_bytes,
            ),
        )?;

        match self.action.get_or_insert(Action::Merge {
            journey_ids: vec![],
        }) {
            Action::Merge { journey_ids } => journey_ids.push(id),
            Action::CompleteRebuilt => (),
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_and_insert_journey(
        &mut self,
        journey_date: NaiveDate,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        created_at: Option<DateTime<Utc>>,
        journey_kind: JourneyKind,
        note: Option<String>,
        journey_data: JourneyData,
    ) -> Result<()> {
        let (journey_data, postprocessor_algo) = match journey_data {
            JourneyData::Vector(journey_vector) => (
                JourneyData::Vector(GpsPostprocessor::process(journey_vector)),
                Some(GpsPostprocessor::current_algo()),
            ),
            JourneyData::Bitmap(bitmap) => (JourneyData::Bitmap(bitmap), None),
        };

        let journey_type = journey_data.type_();
        // create new journey
        let header = JourneyHeader {
            id: Uuid::new_v4().as_hyphenated().to_string(),
            // we use id + revision as the equality check, revision can be any
            // string (e.g. uuid) but a short random should be good enough.
            revision: generate_random_revision(),
            journey_date,
            created_at: created_at.unwrap_or(Utc::now()),
            updated_at: None,
            end,
            start,
            journey_type,
            journey_kind,
            note,
            postprocessor_algo,
        };
        self.insert_journey(header, journey_data)
    }

    pub fn update_journey_metadata(
        &mut self,
        id: &str,
        new_journey_date: NaiveDate,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
        note: Option<String>,
    ) -> Result<()> {
        info!("Updating journey with ID {}", &id);

        let mut header = self
            .get_journey_header(id)?
            .ok_or_else(|| anyhow!("Updating non existent journey, journey id = {}", id))?;

        // must change during update
        header.updated_at = Some(Utc::now());
        header.revision = generate_random_revision();

        let old_journey_date = header.journey_date;
        header.journey_date = new_journey_date;
        header.start = start;
        header.end = end;
        header.note = note;

        // update
        let journey_date = utils::date_to_days_since_epoch(header.journey_date);
        let timestamp_for_ordering = header.start.or(header.end).map(|x| x.timestamp());
        let header_bytes = header.to_proto().write_to_bytes()?;
        let sql = "UPDATE journey SET journey_date = ?1, timestamp_for_ordering = ?2, header = ?3 WHERE id = ?4;";
        self.db_txn.execute(
            sql,
            (journey_date, timestamp_for_ordering, header_bytes, &id),
        )?;

        if old_journey_date != new_journey_date {
            self.action = Some(Action::CompleteRebuilt);
        }

        Ok(())
    }

    pub fn update_journey_data(
        &mut self,
        id: &str,
        journey_data: JourneyData,
        postprocessor_algo: Option<String>,
    ) -> Result<()> {
        info!("Updating journey data with ID {}", &id);

        let mut header = self
            .get_journey_header(id)?
            .ok_or_else(|| anyhow!("Updating non existent journey, journey id = {}", id))?;

        header.postprocessor_algo = postprocessor_algo;

        // must change during update
        header.updated_at = Some(Utc::now());
        header.revision = generate_random_revision();
        header.journey_type = journey_data.type_();

        let header_bytes = header.to_proto().write_to_bytes()?;
        let mut data_bytes = Vec::new();
        journey_data.serialize(&mut data_bytes)?;

        let sql = "UPDATE journey SET type = ?2, header = ?3, data = ?4 WHERE id =?1;";
        self.db_txn.execute(
            sql,
            (&id, journey_data.type_().to_int(), header_bytes, data_bytes),
        )?;

        self.action = Some(Action::CompleteRebuilt);
        Ok(())
    }

    pub fn finalize_ongoing_journey(&mut self) -> Result<bool> {
        let new_journey_added = match self.get_ongoing_journey()? {
            None => false,
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
                    // In practice, `end` could never be none but just in case ...
                    // TODO: Maybe we want better journey date strategy
                    end.unwrap_or(Utc::now()).with_timezone(&Local).date_naive(),
                    start,
                    end,
                    None,
                    journey_kind,
                    None,
                    JourneyData::Vector(journey_vector),
                )?;
                true
            }
        };

        self.db_txn.execute("DELETE FROM ongoing_journey;", ())?;
        self.db_txn.execute(
            "DELETE FROM sqlite_sequence WHERE name='ongoing_journey';",
            (),
        )?;

        info!(
            "Ongoing journey finalized: new_journey_added={}",
            new_journey_added
        );
        Ok(new_journey_added)
    }

    // TODO: we should consider disallow unbounded queries. Keeping all
    // `JourneyHeader` in memory might be a little bit too much.
    // Actually, header is pretty small so it should be fine but still an iterator
    // would be better. Frontend should always use ranged query.
    pub fn query_journeys(
        &self,
        from_date_inclusive: Option<NaiveDate>,
        to_date_inclusive: Option<NaiveDate>,
    ) -> Result<Vec<JourneyHeader>> {
        let mut query = self.db_txn.prepare(
            "SELECT header, type FROM journey WHERE journey_date >= (?1) AND journey_date <= (?2) ORDER BY journey_date DESC, timestamp_for_ordering DESC, id;",
            // use `id` to break tie
        )?;
        let from = match from_date_inclusive {
            None => i32::MIN,
            Some(from_date) => utils::date_to_days_since_epoch(from_date),
        };
        let to = match to_date_inclusive {
            None => i32::MAX,
            Some(to_date) => utils::date_to_days_since_epoch(to_date),
        };
        let mut rows = query.query((from, to))?;
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

    pub fn get_journey_header(&self, id: &str) -> Result<Option<JourneyHeader>> {
        let mut query = self
            .db_txn
            .prepare("SELECT header FROM journey WHERE id = ?1;")?;

        let header_proto_result = query
            .query_row([id], |row| {
                let header_bytes = row.get_ref(0)?.as_blob()?;
                Ok(protos::journey::Header::parse_from_bytes(header_bytes))
            })
            .optional()?;

        match header_proto_result {
            Some(header_proto_result) => {
                let header = JourneyHeader::of_proto(header_proto_result?)?;
                Ok(Some(header))
            }
            None => Ok(None),
        }
    }

    pub fn get_journey_data(&self, id: &str) -> Result<JourneyData> {
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

    pub fn years_with_journey(&self) -> Result<Vec<i32>> {
        let mut query = self
            .db_txn
            .prepare("SELECT DISTINCT CAST(strftime('%Y', journey_date*24*60*60, 'unixepoch') as INTEGER) FROM journey ORDER BY journey_date;")?;
        let mut years: Vec<i32> = Vec::new();
        for row in query.query_map((), |row| row.get(0))? {
            years.push(row?);
        }
        Ok(years)
    }

    pub fn months_with_journey(&self, year: i32) -> Result<Vec<i32>> {
        let mut query = self
            .db_txn
            .prepare("SELECT DISTINCT CAST(strftime('%m', journey_date*24*60*60, 'unixepoch') as INTEGER) FROM journey WHERE strftime('%Y', journey_date*24*60*60, 'unixepoch') = ?1 ORDER BY journey_date;")?;
        let mut months: Vec<i32> = Vec::new();
        for row in query.query_map((format!("{:04}", year),), |row| row.get(0))? {
            months.push(row?);
        }
        Ok(months)
    }

    pub fn days_with_journey(&self, year: i32, month: i32) -> Result<Vec<i32>> {
        let mut query = self
            .db_txn
            .prepare("SELECT DISTINCT CAST(strftime('%d', journey_date*24*60*60, 'unixepoch') as INTEGER) FROM journey WHERE strftime('%Y-%m', journey_date*24*60*60, 'unixepoch') = ?1 ORDER BY journey_date;")?;
        let mut days: Vec<i32> = Vec::new();
        for row in query.query_map((format!("{:04}-{:02}", year, month),), |row| row.get(0))? {
            days.push(row?);
        }
        Ok(days)
    }

    // TODO: consider moving this to `storage.rs`
    pub fn try_auto_finalize_journy(&mut self) -> Result<bool> {
        match self.get_ongoing_journey_timestamp_range()? {
            None => Ok(false),
            Some((start, end)) => {
                // NOTE: this logic is not called very frequently

                let now = Local::now();
                let recording_length_hour = (end.timestamp() - start.timestamp()) / 60 / 60;
                let required_gap_mins = if recording_length_hour >= 72 {
                    0 // let's just finalize it
                } else if recording_length_hour >= 48 {
                    2
                } else if recording_length_hour >= 24 {
                    5
                } else {
                    // if the local date changed since start, we should try to finalize it, otherwise we don't want that unless there is a huge gap (6h)
                    if start.with_timezone(&Local).date_naive() != now.date_naive() {
                        15
                    } else {
                        6 * 60
                    }
                };

                let try_finalize = (now.timestamp() - end.timestamp()) / 60 >= required_gap_mins;

                info!(
                    "Auto finalize ongoing journey: start={}, end={}, now={}, try_finalize={}",
                    start, end, now, try_finalize
                );
                if try_finalize {
                    self.finalize_ongoing_journey()
                } else {
                    Ok(false)
                }
            }
        }
    }

    pub fn earliest_journey_date(&self) -> Result<Option<NaiveDate>> {
        let mut query = self
            .db_txn
            .prepare("SELECT journey_date FROM journey ORDER BY journey_date LIMIT 1;")?;
        Ok(query
            .query_row((), |row| Ok(utils::date_of_days_since_epoch(row.get(0)?)))
            .optional()?)
    }

    pub fn require_optimization(&self) -> Result<bool> {
        let result = self
            .query_journeys(None, None)?
            .iter()
            .any(GpsPostprocessor::outdated_algo);
        if result {
            info!("Main DB require optimization.");
        }
        Ok(result)
    }

    pub fn optimize(&mut self) -> Result<()> {
        info!("Start optimizing main DB.");
        let journey_headers = self.query_journeys(None, None)?;
        for journey_header in journey_headers {
            if GpsPostprocessor::outdated_algo(&journey_header) {
                match self.get_journey_data(&journey_header.id)? {
                    JourneyData::Bitmap(_) => (),
                    JourneyData::Vector(journey_vector) => {
                        let journey_vector = GpsPostprocessor::process(journey_vector);
                        self.update_journey_data(
                            &journey_header.id,
                            JourneyData::Vector(journey_vector),
                            Some(GpsPostprocessor::current_algo()),
                        )?;
                    }
                }
            }
        }
        info!("Done optimizing main DB.");
        Ok(())
    }
}

pub struct MainDb {
    conn: Connection,
}

pub struct OngoingJourney {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub journey_vector: JourneyVector,
}

impl MainDb {
    pub fn open(support_dir: &str) -> MainDb {
        // TODO: better error handling
        let conn = open_db_and_run_migration(
            support_dir,
            "main.db",
            &[&|tx| {
                let sql = "
                CREATE TABLE ongoing_journey (
                    id             INTEGER PRIMARY KEY AUTOINCREMENT
                                        UNIQUE
                                        NOT NULL,
                    timestamp_sec  INTEGER,
                    lat            REAL    NOT NULL,
                    lng            REAL    NOT NULL,
                    process_result INTEGER NOT NULL
                );
                CREATE TABLE journey (
                    id                TEXT    PRIMARY KEY
                                              NOT NULL
                                              UNIQUE,
                    journey_date      INTEGER NOT NULL, -- days since epoch
                    timestamp_for_ordering
                                      INTEGER,          -- start time (fallback to end time)
                    type              INTEGER NOT NULL,
                    header            BLOB    NOT NULL,
                    data              BLOB    NOT NULL
                );
                CREATE INDEX journey_date_index ON journey (
                    journey_date DESC
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
        F: FnOnce(&mut Txn) -> Result<O>,
    {
        let mut txn = Txn {
            db_txn: self.conn.transaction()?,
            action: None,
        };
        let output = f(&mut txn)?;
        txn.db_txn.commit()?;
        Ok(output)
    }

    pub fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }

    /* NOTE:
      Only operations that: do NOT need transactionality AND do NOT affects
      `cache_db` can be put outside `Txn`. Be extra careful.
    */

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
            raw_data.timestamp_ms.map(|x| x / 1000),
            raw_data.point.latitude,
            raw_data.point.longitude,
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
    // TODO: We should consider making the fultter part handle this, similar to
    // `GpsManager.isRecording`.
    RawDataMode,
}

impl Setting {
    fn to_db_key(self) -> &'static str {
        match self {
            Self::RawDataMode => "RAW_DATA_MODE",
        }
    }
}
