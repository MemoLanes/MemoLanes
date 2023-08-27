extern crate simplelog;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::gps_processor::{self, ProcessResult};

pub struct RawDataFile {
    pub name: String,
    pub path: String,
}

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

/* This is an optional feature that should be off by default: storing raw GPS
   data with detailed tempstamp. It will use a new file every time and data are
   written in a simple csv format.

   TODO: we should zstd all old data to reduce disk usage.
*/
struct RawDataRecorder {
    dir: PathBuf,
    file: Option<File>,
}

impl RawDataRecorder {
    fn init(support_dir: &str) -> RawDataRecorder {
        // TODO: better error handling
        let dir = Path::new(support_dir).join("raw_data/");
        std::fs::create_dir_all(&dir).unwrap();
        RawDataRecorder { dir, file: None }
    }

    fn flush(&mut self) {
        if let Some(ref mut file) = self.file {
            file.flush().unwrap()
        }
    }

    fn record(&mut self, raw_data: &gps_processor::RawData, process_result: ProcessResult) {
        // TODO: better error handling
        let file = self.file.get_or_insert_with(|| {
            let timestamp_sec = Utc::now().timestamp_micros() / 1000000;
            let mut i = 0;
            let filename = loop {
                let filename =
                    Path::new(&self.dir).join(format!("gps-{}-{}.csv", timestamp_sec, i));
                if !std::fs::metadata(&filename).is_ok() {
                    break filename;
                }
                i += 1;
            };
            let mut file = File::create(filename).unwrap();
            file.write(
                "timestamp_ms,latitude,longitude,accuarcy,altitude,speed,process_result\n"
                    .as_bytes(),
            )
            .unwrap();
            file
        });
        file.write(
            format!(
                "{},{},{},{},{},{},{}\n",
                raw_data.timestamp_ms,
                raw_data.latitude,
                raw_data.longitude,
                raw_data.accuracy,
                &raw_data.altitude.map(|x| x.to_string()).unwrap_or_default(),
                &raw_data.speed.map(|x| x.to_string()).unwrap_or_default(),
                process_result.to_int()
            )
            .as_bytes(),
        )
        .unwrap();
    }
}

// The main database, we are likely to store a lot of protobuf bytes in it, less relational stuff.
// Basically we will use it as a file system with better transaction support.
struct MainDb {
    conn: Connection,
}

impl MainDb {
    fn open(support_dir: &str) -> MainDb {
        // TODO: better error handling
        let conn =
            open_db_and_run_migration(support_dir, "main.db", /* TODO: migration */ &vec![])
                .expect("failed to open main db");
        MainDb { conn }
    }
}

pub struct Storage {
    support_dir: String,
    main_db: Mutex<MainDb>,
    raw_data_recorder: Mutex<Option<RawDataRecorder>>, // `None` means disabled
}

impl Storage {
    pub fn init(
        _temp_dir: String,
        _doc_dir: String,
        support_dir: String,
        _cache_dir: String,
    ) -> Self {
        let main_db = MainDb::open(&support_dir);
        Storage {
            support_dir,
            main_db: Mutex::new(main_db),
            raw_data_recorder: Mutex::new(None),
        }
    }

    pub fn toggle_raw_data_mode(&self, enable: bool) {
        let mut raw_data_db = self.raw_data_recorder.lock().unwrap();
        if enable {
            if raw_data_db.is_none() {
                *raw_data_db = Some(RawDataRecorder::init(&self.support_dir));
                debug!("[storage] raw data mod enabled");
            }
        } else {
            if raw_data_db.is_some() {
                debug!("[storage] raw data mod disabled");
                // `drop` should do the right thing and release all resources.
                *raw_data_db = None;
            }
        }
    }

    pub fn record_gps_data(
        &self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
    ) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.record(raw_data, process_result);
        }
    }

    pub fn list_all_raw_data(&self) -> Vec<RawDataFile> {
        // TODO: this is way too naive, implement a better one.
        let dir = Path::new(&self.support_dir).join("raw_data/");
        let mut result = Vec::new();
        for path in std::fs::read_dir(dir).unwrap() {
            let file = path.unwrap();
            let filename = file.file_name().to_str().unwrap().to_string();
            if filename.ends_with(".csv") {
                result.push(RawDataFile {
                    name: filename,
                    path: file.path().to_str().unwrap().to_owned(),
                })
            }
        }
        result
    }

    // TODO: do we need this?
    pub fn _flush(&self) -> Result<()> {
        debug!("[storage] flushing");

        let main_db = self.main_db.lock().unwrap();
        main_db.conn.cache_flush()?;
        drop(main_db);

        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.flush();
        }
        drop(raw_data_recorder);

        Ok(())
    }
}
