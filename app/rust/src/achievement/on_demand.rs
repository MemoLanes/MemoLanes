use std::collections::HashMap;

use anyhow::{Context, Result};
use geo_data_format::GeoEntityId;

use crate::achievement::attribution;
use crate::achievement::layer::AchievementLayer;
use crate::achievement::{AchievementReader, GEO_NOT_INSTALLED};
use crate::geo::GeoLookup;
use crate::journey_area_utils::{cm2_to_m2_rounded, journey_bitmap_area_m2_rounded};
use crate::journey_snapshot::JourneySnapshot;

pub fn explored_areas_from_snapshot(
    snapshot: &mut JourneySnapshot,
    layers: &[AchievementLayer],
) -> Result<HashMap<AchievementLayer, u64>> {
    let mut out = HashMap::with_capacity(layers.len());
    for &layer in layers {
        let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
        out.insert(layer, journey_bitmap_area_m2_rounded(&bitmap, None));
    }
    Ok(out)
}

pub fn region_areas_from_snapshot(
    snapshot: &mut JourneySnapshot,
    geo: &dyn GeoLookup,
    layer: AchievementLayer,
    ids: &[GeoEntityId],
) -> Result<HashMap<GeoEntityId, u64>> {
    let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
    let by_entity = attribution::attribute(&bitmap, geo)?;
    Ok(ids
        .iter()
        .filter_map(|id| by_entity.get(id).map(|&cm2| (*id, cm2_to_m2_rounded(cm2))))
        .collect())
}

pub struct OnDemandReader<'a, 'snap> {
    snapshot: JourneySnapshot<'a, 'snap>,
    geo: Option<&'a dyn GeoLookup>,
}

impl<'a, 'snap> OnDemandReader<'a, 'snap> {
    pub fn new(snapshot: JourneySnapshot<'a, 'snap>, geo: Option<&'a dyn GeoLookup>) -> Self {
        OnDemandReader { snapshot, geo }
    }
}

impl AchievementReader for OnDemandReader<'_, '_> {
    fn explored_area_m2(&mut self, layer: AchievementLayer) -> Result<u64> {
        Ok(explored_areas_from_snapshot(&mut self.snapshot, &[layer])?
            .remove(&layer)
            .unwrap_or(0))
    }

    fn region_areas(
        &mut self,
        layer: AchievementLayer,
        ids: &[GeoEntityId],
    ) -> Result<HashMap<GeoEntityId, u64>> {
        // No geo → no regions (empty until a worldview is installed).
        let Some(geo) = self.geo else {
            return Ok(HashMap::new());
        };
        region_areas_from_snapshot(&mut self.snapshot, geo, layer, ids)
    }

    fn geo(&self) -> Result<&dyn GeoLookup> {
        self.geo.context(GEO_NOT_INSTALLED)
    }
}
