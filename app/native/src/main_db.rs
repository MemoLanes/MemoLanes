extern crate simplelog;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::gps_processor::{self, ProcessResult};

/*  The main database, we are likely to store a lot of protobuf bytes in it,
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
    if version < target_version {
        for i in (version)..target_version {
            info!("running migration for version: {}", i + 1);
            let f = migrations.get(i).unwrap();
            f(&tx)?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO `db_metadata` (key, value) VALUES (?1, ?2)",
            ("version", target_version.to_string()),
        )?;
    } else if version > target_version {
        bail!(
            "version too high: current version = {}, target_version = {}",
            version,
            target_version
        );
    }
    tx.commit()?;
    return Ok(conn);
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
                    end_time DESC
                );
                ";
                tx.execute(sql, ())?;
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
        tx.execute(
            sql,
            (
                raw_data.timestamp_ms / 1000,
                raw_data.latitude,
                raw_data.longitude,
                process_result,
            ),
        )?;
        Ok(())
    }

    fn finalize_ongoing_journey(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn record(
        &mut self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
    ) -> Result<()> {
        match process_result {
            ProcessResult::Ignore => (),
            ProcessResult::NewJourney => {
                self.finalize_ongoing_journey()?;
                self.append_ongoing_journey(raw_data, ProcessResult::Append)?;
            }
            ProcessResult::Append | ProcessResult::NewSegment => {
                self.append_ongoing_journey(raw_data, process_result)?;
            }
        }
        Ok(())
    }
}
