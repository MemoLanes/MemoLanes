use chrono::NaiveDate;
use rusqlite::Connection;
use std::path::Path;

use crate::{
    achievement::AchievementReader, geo::GeoLookup, journey_bitmap::JourneyBitmap,
    journey_data::JourneyData, journey_header::JourneyKind, main_db, utils,
};

mod bitmap_io;
mod full_table;
mod range;

mod v1;
pub use v1::CacheDbV1;

use anyhow::Result;

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub struct CacheEntry {
    pub date: NaiveDate,
    pub kind: JourneyKind,
}

/// flutter_rust_bridge:ignore
#[derive(Eq, Hash, Clone, Copy, Debug, PartialEq)]
pub enum LayerKind {
    All,
    JourneyKind(JourneyKind),
}

impl LayerKind {
    fn to_sql(self) -> &'static str {
        match self {
            LayerKind::All => "All",
            LayerKind::JourneyKind(kind) => match kind {
                JourneyKind::DefaultKind => "Default",
                JourneyKind::Flight => "Flight",
            },
        }
    }
}

/// Open `cache_dir/cache.db` and bring it up to the latest migration. A cache
/// left behind by a newer major version is discarded and recreated: everything
/// in it can be recomputed from the main db.
fn open_or_recreate(
    cache_dir: &str,
    migrations: &[utils::db::Migration<'_>],
) -> anyhow::Result<Connection> {
    match utils::db::open_and_migrate(cache_dir, "cache.db", migrations) {
        Err(error)
            if matches!(
                error.downcast_ref::<utils::db::DbError>(),
                Some(utils::db::DbError::VersionTooNew)
            ) =>
        {
            warn!("discarding incompatible cache db");
            std::fs::remove_file(Path::new(cache_dir).join("cache.db"))?;
            utils::db::open_and_migrate(cache_dir, "cache.db", migrations)
        }
        result => result,
    }
}

pub trait CacheDb {
    /// Get or compute the merged bitmap for `layer_kind`.
    ///
    /// - `range: None` → full (all-time) range, served from and written
    ///   to the cache.
    /// - `range: Some((from, to))` → that inclusive window, computed
    ///   directly from the main DB (not cached).
    fn get_or_compute(
        &mut self,
        txn: &main_db::Txn,
        layer_kind: &LayerKind,
        range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<JourneyBitmap>;

    fn merge_journey(
        &mut self,
        entry: &CacheEntry,
        data: &JourneyData,
        geo: Option<&dyn GeoLookup>,
    ) -> Result<()>;

    /// Invalidate cached data for the given entries and all affected aggregates.
    ///
    /// Clears cached data covering the month of each entry's date, for both
    /// the entry's kind and `LayerKind::All`. Also clears any aggregate entries.
    fn invalidate(&mut self, entries: &[CacheEntry]) -> Result<()>;

    fn clear_all(&mut self) -> Result<()>;
    fn flush(&self) -> Result<()>;

    fn achievement_reader<'a>(
        &'a mut self,
        txn: &'a main_db::Txn<'_>,
        geo: Option<&'a dyn GeoLookup>,
    ) -> Result<Box<dyn AchievementReader + 'a>>;

    // TODO: add a function to populate/optimize the cache after invalidation/merging
    // to improve UX after add/edit/delete large amount of data.
}

pub fn new(cache_dir: &str) -> impl CacheDb {
    CacheDbV1::open(cache_dir)
}
