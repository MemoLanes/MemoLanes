//! The achievement-store traits: a pair the storage/api layers consume without
//! binding to a concrete store. The compute-on-demand
//! [`OnDemandStore`](super::backend::on_demand::OnDemandStore) implements them.
//! Mirrors `cache_db`'s `CacheDb` trait + `new()` factory.

use anyhow::Result;
use geo_data_format::Worldview;

use crate::achievement::compute::region_state::RegionStateMap;
use crate::achievement::layer::AchievementLayer;
use crate::geo::GeoLookup;
use crate::journey_snapshot::JourneySnapshot;

/// The long-lived achievement engine owned by `Storage.dbs`. Holds the active
/// worldview geo.
pub trait AchievementStore: Send {
    /// Notify of a committed journey change. This store recomputes per read, so
    /// it's a no-op; a cached implementation would invalidate here.
    fn invalidate_all(&self) -> Result<()>;

    /// Supply (or switch) the active worldview's geo lookup, kept for region reads.
    /// A worldview change re-derives worldview-scoped state.
    fn set_geo(&mut self, worldview: Worldview, geo: Box<dyn GeoLookup + Send>) -> Result<()>;

    /// Open a reader over `snapshot`: computes lazily, borrowing both `self`
    /// (geo) and `snapshot`, so every value it serves reflects one snapshot.
    fn reader<'a>(
        &'a self,
        snapshot: &'a JourneySnapshot,
    ) -> Result<Box<dyn AchievementReader + 'a>>;
}

/// The read surface the api layer consumes, under one snapshot.
pub trait AchievementReader {
    /// Explored area (m²) for one layer (0 if absent).
    fn explored_area_m2(&self, layer: AchievementLayer) -> Result<u64>;

    /// Region states for the active worldview, restricted to `layers`. Scoping the
    /// query lets the hot single-layer reads avoid computing the others.
    fn region_states(&self, layers: &[AchievementLayer]) -> Result<RegionStateMap>;

    // TODO: It is pretty awkward that we need to deal with `geo` being `None`
    // in all places. However, in fact, this has to always be `Some` due to
    // `app_bootstrap.dart`.
    // So we should do either: 1. treat `None` as an error. 2. think about a
    // better way and support lazy init.
    /// The active worldview's geo lookup, or `None` until `set_geo`.
    fn geo(&self) -> Option<&dyn GeoLookup>;
}
