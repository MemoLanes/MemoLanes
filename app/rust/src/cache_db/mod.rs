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
    pub(crate) fn to_sql(self) -> &'static str {
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
///
/// **Thread-safety contract**: Implementations assume single-threaded access.
/// The caller (e.g. `Storage`) must hold a `Mutex` to prevent concurrent access.
pub trait CacheDb {
    /// Get or compute a bitmap for the given date range.
    ///
    /// - `from: None, to: None` → full range
    /// - `from: Some, to: Some` → explicit range
    ///
    /// Returns cached data when available; otherwise computes from the main DB
    /// via `txn` and caches the results.
    fn get_or_compute(
        &self,
        txn: &main_db::Txn,
        layer_kind: &LayerKind,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<JourneyBitmap>;

    /// Incrementally merge new journey data into the cache.
    ///
    /// If a cache entry covering `entry`'s date exists: merges `data` into it.
    /// If none exists: no-op.
    /// Always invalidates any aggregate full-table entries for the given kind
    /// and `LayerKind::All`.
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
