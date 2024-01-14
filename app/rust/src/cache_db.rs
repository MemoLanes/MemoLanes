extern crate simplelog;
use rusqlite::{Connection, OptionalExtension, Transaction};
use anyhow::{Result, bail};
use std::cmp::Ordering;
use std::path::Path;
use protobuf::Message;

use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};

// Function to open the database and run migrations
fn open_db_and_run_migration(
    cache_dir: &str,
    file_name: &str,
    migrations: &Vec<&dyn Fn(&Transaction) -> Result<()>>,
) -> Result<Connection> {
    debug!("open and run migration for {}", file_name);
    let mut conn = Connection::open(Path::new(cache_dir).join(file_name))?;
    let tx = conn.transaction()?;
    let create_db_metadata_sql = "
        CREATE TABLE IF NOT EXISTS `db_metadata` (
            `key`    TEXT NOT NULL,
            `value`  TEXT,
            PRIMARY KEY(`key`)
        )";
    tx.execute(create_db_metadata_sql, [])?;

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
            for i in version..target_version {
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

// Transaction wrapper
pub struct Txn<'a> {
    db_txn: rusqlite::Transaction<'a>,
}

impl Txn<'_> {
    // Method to insert journey bitmap blob
    pub fn insert_journey_bitmap_blob(
        &self,
        date_bytes: Vec<u8>,
    ) -> Result<()> {
        let sql = "INSERT INTO journey_cache (data) VALUES (?1);";
        self.db_txn.execute(
            sql,
            &[&date_bytes],
        )?;
        Ok(())
    }

    // Method to get the last journey
    pub fn get_journey(&self) -> Result<JourneyData> {
        let mut query = self
            .db_txn
            .prepare("SELECT data FROM journey_cache ORDER BY id DESC LIMIT 1;")?;
    
        query.query_row((), |row| {
            let type_ = row.get_ref(0)?.as_i64()?;
            let f = || {
                let data = row.get_ref(1)?.as_blob()?;
                JourneyData::deserialize(data, JourneyType::Bitmap)
            };
            Ok(f())
        })?
    }
    
}

// CacheDb structure
pub struct CacheDb {
    conn: Connection,
}

impl CacheDb {
    // Method to open and return a CacheDb instance
    pub fn open(cache_dir: &str) -> CacheDb{
        let conn = open_db_and_run_migration(
            cache_dir,
            "cache.db",  // Corrected file name
            &vec![&|tx| {
                let sql = "
                    CREATE TABLE journey_cache (
                        id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL UNIQUE,
                        data BLOB NOT NULL
                    );
                    CREATE TABLE setting (
                        key TEXT PRIMARY KEY NOT NULL UNIQUE,
                        value TEXT
                    );
                ";
                for s in sql_split::split(sql) {
                    tx.execute(&s, [])?;
                }
                Ok(())
            }],
        )
        .expect("failed to open main db");
        CacheDb { conn }
    }

    // Method to execute a transaction
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

    // Method to flush the cache
    pub fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }
}
