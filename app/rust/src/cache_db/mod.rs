use chrono::NaiveDate;

use crate::{
    journey_bitmap::JourneyBitmap, journey_data::JourneyData, journey_header::JourneyKind, main_db,
};

mod cache_db_v1;
pub use cache_db_v1::CacheDbV1;

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
    pub(self) fn to_sql(self) -> &'static str {
        match self {
            LayerKind::All => "All",
            LayerKind::JourneyKind(kind) => match kind {
                JourneyKind::DefaultKind => "Default",
                JourneyKind::Flight => "Flight",
            },
        }
    }
}

/// Cache for merged journey bitmaps.
pub trait CacheDb {
    /// Get or compute the merged bitmap for `layer_kind`.
    ///
    /// - `range: None` → full (all-time) range, served from and written
    ///   to the cache.
    /// - `range: Some((from, to))` → that inclusive window, computed
    ///   directly from the main DB (not cached).
    fn get_or_compute(
        &self,
        txn: &main_db::Txn,
        layer_kind: &LayerKind,
        range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<JourneyBitmap>;

    /// Incrementally merge new journey data into the cache.
    fn merge_journey(&self, entry: &CacheEntry, data: &JourneyData) -> Result<()>;

    /// Invalidate cached data for the given entries and all affected aggregates.
    ///
    /// Clears cached data covering the month of each entry's date, for both
    /// the entry's kind and `LayerKind::All`. Also clears any aggregate entries.
    fn invalidate(&self, entries: &[CacheEntry]) -> Result<()>;

    fn clear_all(&self) -> Result<()>;
    fn flush(&self) -> Result<()>;

    // TODO: add a function to populate/optimize the cache after invalidation/merging
    // to improve UX after add/edit/delete large amount of data.
}

pub fn new(cache_dir: &str) -> impl CacheDb {
    CacheDbV1::open(cache_dir)
}
