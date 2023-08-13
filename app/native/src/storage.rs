extern crate simplelog;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::path::Path;
use std::sync::Mutex;

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

// This is an optional expert feature that should be off by default: storing raw GPS data with detailed tempstamp.
struct RawDataDb {
    conn: Mutex<Connection>,
}

impl RawDataDb {
    fn open(support_dir: &str) -> RawDataDb {
        // better error handling
        let conn = open_db_and_run_migration(support_dir, "raw_data.db", &vec![&(|tx| Ok(()))])
            .expect("failed to open raw data db");
        RawDataDb {conn : Mutex::new(conn)}
    }
}

pub struct Storage {
    _temp_dir: String, // Currently unused
    _doc_dir: String,  // Currently unused
    support_dir: String,
    cache_dir: String,
    raw_data_db: Option<RawDataDb> // `None` means disabled
}

impl Storage {
    pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) -> Self {
        let raw_data_db = RawDataDb::open(&support_dir);
        Storage {
            _temp_dir: temp_dir,
            _doc_dir: doc_dir,
            support_dir,
            cache_dir,
            // TODO: make this `None` by default
            raw_data_db: Some(raw_data_db), 
        }
    }
}
