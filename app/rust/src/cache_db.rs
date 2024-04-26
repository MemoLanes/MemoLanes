extern crate simplelog;
use anyhow::Result;
use rusqlite::{params, Connection, Error as RusqliteError};
use std::path::Path;
use std::io::Cursor;

use crate::journey_data::deserialize_journey_bitmap;
use crate::journey_bitmap::JourneyBitmap;

// Function to open the database and run migrations
#[allow(clippy::type_complexity)]
fn open_db(cache_dir: &str, file_name: &str, sql: &str) -> Result<Connection> {
    debug!("opening cache db for {}", file_name);
    let conn = Connection::open(Path::new(cache_dir).join(file_name))?;
    conn.execute(sql, [])?;
    Ok(conn)
}

// CacheDb structure
pub struct CacheDb {
    conn: Connection,
}

impl CacheDb {
    // Method to open and return a CacheDb instance
    pub fn open(cache_dir: &str) -> CacheDb {
        let conn = open_db(
            cache_dir,
            "cache.db",
            "CREATE TABLE IF NOT EXISTS journey_cache (
                        id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL UNIQUE,
                        data BLOB NOT NULL
                    );",
        )
        .expect("failed to open cache db");
        CacheDb { conn }
    }

    // Method to flush the cache
    pub fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }

    pub fn get_journey(&self) -> Result<Option<JourneyBitmap>> {
        let query = "SELECT * FROM journey_cache ORDER BY id DESC LIMIT 1;";
        let result = self.conn.query_row(query, params![], |row| {
            let _journey_type = row.get_ref(0)?.as_i64()?;
            let data = row.get_ref(1)?.as_blob()?;
            Ok(data.to_vec())
        });

        match result {
            Ok(data) => {
                let cursor = Cursor::new(data);
                let journey_bitmap = deserialize_journey_bitmap(cursor);
                match journey_bitmap {
                    Ok(bitmap) => Ok(Some(bitmap)),
                    Err(e) => Err(e.into())
                }
            },
            Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // Method to insert journey bitmap blob
    pub fn insert_journey_bitmap(&self, date_bytes: Vec<u8>) -> Result<()> {
        let sql = "INSERT INTO journey_cache (data) VALUES (?1);";
        self.conn.execute(sql, [&date_bytes])?;

        Ok(())
    }

    pub fn delete_cached_journey(&self) -> Result<()> {
        // TODO: in v1, use more fine-grained delete with year/month
        let sql = "DELETE FROM journey_cache;";
        self.conn.execute(sql, [])?;

        Ok(())
    }
}
