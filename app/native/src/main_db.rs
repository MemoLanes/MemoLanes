extern crate simplelog;
use anyhow::Result;
use protobuf::{Message, MessageField};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::cmp::Ordering;
use std::path::Path;
use uuid::Uuid;

use crate::gps_processor::{self, ProcessResult};
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

pub struct MainDb {
    conn: Connection,
}

impl MainDb {
    pub fn open(support_dir: &str) -> MainDb {
        // TODO: better error handling
        let conn = open_db_and_run_migration(
            support_dir,
            "main.db",
            /* TODO: migration */
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
                    header            BLOB    NOT NULL,
                    data              BLOB    NOT NULL
                );
                CREATE INDEX end_time_index ON journey (
                    end_timestamp_sec DESC
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

    pub fn finalize_ongoing_journey(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        // `id` in `ongoing_journey` is auto incremented.
        let mut query = tx.prepare(
            "SELECT timestamp_sec, lat, lng, process_result FROM ongoing_journey ORDER BY id;",
        )?;
        let results = query.query_map((), |row| {
            let timestamp_sec: i64 = row.get(0)?;
            let process_result: i8 = row.get(3)?;
            let mut track_point = protos::journey::data::TrackPoint::new();
            track_point.latitude = row.get(1)?;
            track_point.longitude = row.get(2)?;
            Ok((timestamp_sec, track_point, process_result))
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
                let mut track_segmant = protos::journey::data::TrackSegmant::new();
                track_segmant.track_points = current_segment;
                segmants.push(track_segmant);
                current_segment = Vec::new();
            }
            current_segment.push(track_point);
        }
        if !current_segment.is_empty() {
            let mut track_segmant = protos::journey::data::TrackSegmant::new();
            track_segmant.track_points = current_segment;
            segmants.push(track_segmant);
        }

        drop(query);

        // create new journey
        if !segmants.is_empty() {
            let end_timestamp_sec = end_timestamp_sec.unwrap();

            let mut header = protos::journey::Header::new();
            header.id = Uuid::new_v4().as_hyphenated().to_string();
            header.end_timestamp_sec = end_timestamp_sec;
            header.start_timestamp_sec = start_timestamp_sec;
            // TODO: allow user to set this when recording?
            let mut kind = protos::journey::header::Kind::new();
            kind.set_build_in(protos::journey::header::kind::BuiltIn::DEFAULT);
            header.kind = MessageField::some(kind);
            header.note = None;

            let mut track = protos::journey::data::Track::new();
            track.track_segmants = segmants;
            let mut data = protos::journey::Data::new();
            data.set_track(track);

            // TODO: we could have some additional post-processing of the track.
            // including path refinement + lossy compression.

            // TODO: before we stabilize the desgin, see what's the pro/con of
            // compressing this with zstd. Naively I think our data is pretty
            // compressible.
            let header_bytes = header.write_to_bytes()?;
            let data_bytes = data.write_to_bytes()?;

            let sql = "INSERT INTO journey (id, end_timestamp_sec, header, data) VALUES (?1, ?2, ?3, ?4);";
            tx.execute(
                sql,
                (&header.id, end_timestamp_sec, header_bytes, data_bytes),
            )?;
        }

        tx.execute("DELETE FROM ongoing_journey;", ())?;

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
}
