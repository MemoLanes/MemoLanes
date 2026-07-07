//! Compute-on-demand achievement store: no cache, no persistence. Holds only
//! the active worldview geo and computes every read from the journey snapshot via the
//! pure functions in this module.

use anyhow::Result;
use geo_data_format::Worldview;

use crate::achievement::compute::explored_area::explored_areas_from_snapshot;
use crate::achievement::compute::region_state::{compute_region_states, RegionStateMap};
use crate::achievement::contract::{AchievementReader, AchievementStore};
use crate::achievement::layer::AchievementLayer;
use crate::geo::GeoLookup;
use crate::journey_snapshot::JourneySnapshot;

#[derive(Default)]
pub struct OnDemandStore {
    geo: Option<Box<dyn GeoLookup + Send>>,
}

impl OnDemandStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AchievementStore for OnDemandStore {
    /// No persisted state to invalidate — every read recomputes from source.
    fn invalidate_all(&self) -> Result<()> {
        Ok(())
    }

    /// No persistence/cache, so the worldview id isn't needed — only the geo
    /// lookup drives reads.
    fn set_geo(&mut self, _worldview: Worldview, geo: Box<dyn GeoLookup + Send>) -> Result<()> {
        self.geo = Some(geo);
        Ok(())
    }

    fn reader<'a>(
        &'a self,
        snapshot: &'a JourneySnapshot,
    ) -> Result<Box<dyn AchievementReader + 'a>> {
        Ok(Box::new(OnDemandReader {
            snapshot,
            geo: self.geo.as_deref().map(|g| g as &dyn GeoLookup),
        }))
    }
}

/// One reader bound to a snapshot; computes on demand from the shared pure fns.
struct OnDemandReader<'a, 'snap, 'txn> {
    snapshot: &'a JourneySnapshot<'snap, 'txn>,
    geo: Option<&'a dyn GeoLookup>,
}

impl AchievementReader for OnDemandReader<'_, '_, '_> {
    fn explored_area_m2(&self, layer: AchievementLayer) -> Result<u64> {
        Ok(explored_areas_from_snapshot(self.snapshot, &[layer])?
            .remove(&layer)
            .unwrap_or(0))
    }

    fn region_states(&self, layers: &[AchievementLayer]) -> Result<RegionStateMap> {
        // No geo → no regions (empty until `set_geo`).
        let Some(geo) = self.geo else {
            return Ok(RegionStateMap::new());
        };
        // Only the requested layers are computed — Default reads never pay for
        // Flight/All.
        let layer_bitmaps = layers
            .iter()
            .map(|&layer| {
                Ok((
                    layer,
                    self.snapshot
                        .finalized_bitmap(&layer.to_layer_kind(), None)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(compute_region_states(layer_bitmaps, geo))
    }

    fn geo(&self) -> Option<&dyn GeoLookup> {
        self.geo
    }
}
