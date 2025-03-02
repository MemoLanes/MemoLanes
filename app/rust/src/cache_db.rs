extern crate simplelog;
use anyhow::{Ok, Result};
use rusqlite::{Connection, OptionalExtension};
use std::cmp::Ordering;
use std::path::Path;

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

pub const TARGET_VERSION: i32 = 2;

fn open_db(cache_dir: &str, file_name: &str, sql: &str) -> Result<Connection> {
    // TODO: maybe we want versioning or at least detect versioning issue
    // so we could rebuilt it.
    debug!("opening cache db for {}", file_name);
    let mut conn = Connection::open(Path::new(cache_dir).join(file_name))?;

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

    let target_version = TARGET_VERSION;
    debug!(
        "current version = {}, target_version = {}",
        version, target_version
    );
    match version.cmp(&target_version) {
        Ordering::Equal => (),
        Ordering::Less => {
            tx.execute("DROP TABLE IF EXISTS journey_cache;", ());
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

    tx.execute(sql, [])?;
    tx.commit();
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
            "CREATE TABLE IF NOT EXISTS `journey_cache` (
                        kind Text PRIMARY KEY NOT NULL UNIQUE,
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

    fn get_serialization(kind: Option<&JourneyKind>) -> &str {
        match kind {
            None => "ALL",
            Some(JourneyKind::DefaultKind) => "DefaultKind",
            Some(JourneyKind::Flight) => "Flight",
        }
    }

    // Format SQL, query the database and returns rows of result
    fn get_journey_cache(
        &self,
        journey_kind: Option<&JourneyKind>,
    ) -> Result<Option<JourneyBitmap>> {
        let kind = Self::get_serialization(journey_kind);
        let sql = "SELECT data FROM `journey_cache` WHERE kind = ?1;";

        let mut query = self.conn.prepare(sql)?;
        let data = query
            .query_row((kind,), |row| {
                let data = row.get_ref(0)?.as_blob()?;
                Ok(journey_data::deserialize_journey_bitmap(data))
            })
            .optional()?
            .transpose()?;

        Ok(data)
    }

    fn set_journey_cache(
        &self,
        journey_kind: Option<&JourneyKind>,
        journey_bitmap: &JourneyBitmap,
    ) -> Result<()> {
        let kind = Self::get_serialization(journey_kind);
        let mut data = Vec::new();
        journey_data::serialize_journey_bitmap(journey_bitmap, &mut data)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO `journey_cache` (kind, data) VALUES (?1, ?2, ?3)",
            (&kind, &data),
        )?;
        Ok(())
    }

    // get a merged cache or compute from storage
    pub fn get_journey_cache_or_compute<F>(
        &self,
        kind: Option<&JourneyKind>, // Default, Flight, None
        f: F,
    ) -> Result<JourneyBitmap>
    where
        F: FnOnce() -> Result<JourneyBitmap>,
    {
        match self.get_journey_cache(kind)? {
            Some(journey_bitmap) => Ok(journey_bitmap),
            None => {
                let journey_bitmap = f()?;
                self.set_journey_cache(kind, &journey_bitmap)?;

                Ok(journey_bitmap)
            }
        }
    }

    pub fn clear_journey_cache(&self) -> Result<()> {
        // TODO: in v3, use more fine-grained delete with year/month
        self.conn.execute("DELETE FROM journey_cache;", [])?;
        Ok(())
    }

    pub fn upsert_journey_cache(
        &self,
        kind: &JourneyKind, // Default or Flight only
        journey: JourneyData,
    ) -> Result<()> {
        let mut journey_bitmap = match self.get_journey_cache(Some(kind))? {
            Some(cache_bitmap) => cache_bitmap,
            None => return Ok(()),
        };

        match journey {
            JourneyData::Vector(vector) => {
                add_journey_vector_to_journey_bitmap(&mut journey_bitmap, &vector)
            }
            JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
        }

        // Update the journey cache with the new or merged bitmap
        self.set_journey_cache(Some(kind), &journey_bitmap)
    }
}
