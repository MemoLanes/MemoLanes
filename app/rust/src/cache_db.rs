extern crate simplelog;
use anyhow::Result;
use protobuf::Message;
use rusqlite::Connection;
use std::{collections::HashMap, path::Path};

use crate::{
    journey_bitmap::JourneyBitmap,
    journey_data::{self, JourneyData},
    journey_header::JourneyKind,
    merged_journey_builder::add_journey_vector_to_journey_bitmap,
};

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
    All, // deprecated in the future
         // Add more for ALL_YEAR_MONTH
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
                        kind Text,
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

    // Format SQL, query the database and returns rows of result
    fn get_journey_cache(
        &self,
        key: &JourneyCacheKey,
        kind: Option<&JourneyKind>,
    ) -> Result<Vec<Option<JourneyBitmap>>> {
        let key_cond = key.to_db_string();
        let kind_string = kind.map(|kind_value| kind_value.clone().to_proto().to_string());

        let (sql, params): (&str, Vec<&dyn rusqlite::ToSql>) = match kind_string {
            Some(ref kind_str) => (
                "SELECT data FROM `journey_cache` WHERE key = ?1 AND kind = ?2;",
                vec![&key_cond, kind_str],
            ),
            None => (
                "SELECT data FROM `journey_cache` WHERE key = ?1;",
                vec![&key_cond],
            ),
        };

        let mut query = self.conn.prepare(sql)?;
        let data: Vec<Option<JourneyBitmap>> = query
            .query_map(&params[..], |row| {
                let data = row.get_ref("data")?.as_blob()?;
                journey_data::deserialize_journey_bitmap(data)
                    .map(Some)
                    .or_else(|_| Ok(None))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(data)
    }

    // For now simply merge all kinds, since they are under ALL
    fn get_merged_journey_cache(
        &self,
        key: &JourneyCacheKey,
        kind: Option<&JourneyKind>,
    ) -> Result<Option<JourneyBitmap>> {
        let res = self.get_journey_cache(key, kind)?;
        let mut merged_bitmap = JourneyBitmap::new();

        for journey_bitmap in res {
            match journey_bitmap {
                Some(bitmap) => merged_bitmap.merge(bitmap),
                None => continue,
            }
        }

        if merged_bitmap.tiles.is_empty() {
            Ok(None)
        } else {
            Ok(Some(merged_bitmap))
        }
    }

    fn set_journey_cache(
        &self,
        key: &JourneyCacheKey,
        journey_kind: JourneyKind,
        journey_bitmap: &JourneyBitmap,
    ) -> Result<()> {
        let kind = journey_kind.to_proto().write_to_bytes()?;
        let mut data = Vec::new();
        journey_data::serialize_journey_bitmap(journey_bitmap, &mut data)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO `journey_cache` (key, kind, data) VALUES (?1, ?2, ?3)",
            (key.to_db_string(), &kind, &data),
        )?;
        Ok(())
    }

    // get a merged cache or compute from storage
    pub fn get_journey_cache_or_compute<F>(
        &self,
        key: &JourneyCacheKey,
        f: F,
    ) -> Result<JourneyBitmap>
    where
        F: FnOnce() -> Result<HashMap<JourneyKind, JourneyBitmap>>,
    {
        match self.get_merged_journey_cache(key, None)? {
            Some(journey_bitmap) => Ok(journey_bitmap),
            None => {
                let journey_hashmap = f()?;
                let mut merged_bitmap = JourneyBitmap::new();
                for (journey_kind, journey_bitmap) in journey_hashmap {
                    self.set_journey_cache(key, journey_kind, &journey_bitmap)?;
                    merged_bitmap.merge(journey_bitmap);
                }
                Ok(merged_bitmap)
            }
        }
    }

    pub fn clear_journey_cache(&self) -> Result<()> {
        // TODO: in v1, use more fine-grained delete with year/month
        self.conn.execute("DELETE FROM journey_cache;", [])?;
        Ok(())
    }

    pub fn upsert_journey_cache(
        &self,
        key: &JourneyCacheKey,
        kind: JourneyKind,
        journey: JourneyData,
    ) -> Result<()> {
        let mut journey_bitmap = match self.get_merged_journey_cache(key, Some(&kind))? {
            Some(cache_bitmap) => cache_bitmap,
            None => JourneyBitmap::new(),
        };

        match journey {
            JourneyData::Vector(vector) => {
                add_journey_vector_to_journey_bitmap(&mut journey_bitmap, &vector)
            }
            JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
        }

        // Update the journey cache with the new or merged bitmap
        self.set_journey_cache(key, kind, &journey_bitmap)
    }
}
