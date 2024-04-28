extern crate simplelog;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

use crate::{journey_bitmap::JourneyBitmap, journey_data};

// TODO: Right now, we keep a cache of all finalized journeys (and fallback to
// compute it by merging all journeys in main db). We clear this cache entirely
// when there is a update to any finalized journey in main db.
// To improve this, we should:
// V1: we want more fine-grained cache invalidation/update rules. e.g. If we are
// only append new finalized journey, then we could just merge that single
// journey with the existing cache.
// V2: we might need multiple caches for different layer (e.g. one for flight,
// one for land).
// V3: we might need multiple caches keyed by `(start_time, end_time]`, and
// have one per year or one per month. So when there is an update to the
// history, we could clear some but not all cache, and re-construct these
//  outdated ones reasonably quickly.

fn open_db(cache_dir: &str, file_name: &str, sql: &str) -> Result<Connection> {
    // TODO: maybe we want versioning or at least detect versioning issue
    // so we could rebuilt it.
    debug!("opening cache db for {}", file_name);
    let conn = Connection::open(Path::new(cache_dir).join(file_name))?;
    conn.execute(sql, [])?;
    Ok(conn)
}

#[derive(Clone, Debug)]
pub enum JourneyCacheKey {
    All,
}

impl JourneyCacheKey {
    fn to_db_string(&self) -> String {
        match self {
            Self::All => "A".to_owned(),
        }
    }

    fn _of_db_string(str: &str) -> Self {
        match str {
            "A" => Self::All,
            _ => panic!("Invalid `JourneyCacheKey`, str = {}", str),
        }
    }
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
            "CREATE TABLE IF NOT EXISTS `journey_cache` (
                        key TEXT PRIMARY KEY NOT NULL UNIQUE,
                        data BLOB NOT NULL
                    );",
        )
        .expect("failed to open cache db");
        CacheDb { conn }
    }

    pub fn flush(&self) -> Result<()> {
        self.conn.cache_flush()?;
        Ok(())
    }

    fn get_journey_cache(&self, key: &JourneyCacheKey) -> Result<Option<JourneyBitmap>> {
        let mut query = self
            .conn
            .prepare("SELECT data FROM `journey_cache` WHERE key = ?1;")?;

        let data = query
            .query_row([key.to_db_string()], |row| {
                let data = row.get_ref(0)?.as_blob()?;
                Ok(journey_data::deserialize_journey_bitmap(data))
            })
            .optional()?;

        match data {
            None => Ok(None),
            Some(journey_bitmap) => Ok(Some(journey_bitmap?)),
        }
    }

    fn set_journey_cache(
        &self,
        key: &JourneyCacheKey,
        journey_bitmap: &JourneyBitmap,
    ) -> Result<()> {
        let mut data = Vec::new();
        journey_data::serialize_journey_bitmap(journey_bitmap, &mut data)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO `journey_cache` (key, data) VALUES (?1, ?2)",
            (key.to_db_string(), &data),
        )?;
        Ok(())
    }

    pub fn get_journey_cache_or_compute<F>(
        &self,
        key: &JourneyCacheKey,
        f: F,
    ) -> Result<JourneyBitmap>
    where
        F: FnOnce() -> Result<JourneyBitmap>,
    {
        match self.get_journey_cache(key)? {
            Some(journey_bitmap) => Ok(journey_bitmap),
            None => {
                let journey_bitmap = f()?;
                self.set_journey_cache(key, &journey_bitmap)?;
                Ok(journey_bitmap)
            }
        }
    }

    pub fn clear_journey_cache(&self) -> Result<()> {
        // TODO: in v1, use more fine-grained delete with year/month
        self.conn.execute("DELETE FROM journey_cache;", [])?;
        Ok(())
    }
}
