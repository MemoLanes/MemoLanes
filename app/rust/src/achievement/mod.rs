pub mod attribution;
pub mod layer;
pub mod on_demand;
pub mod region;

use std::collections::HashMap;

use anyhow::Result;
use geo_data_format::GeoEntityId;

use crate::achievement::layer::AchievementLayer;
use crate::geo::GeoLookup;

pub(crate) const GEO_NOT_INSTALLED: &str = "geo not installed";

pub trait AchievementReader {
    fn explored_area_m2(&mut self, layer: AchievementLayer) -> Result<u64>;

    fn region_areas(
        &mut self,
        layer: AchievementLayer,
        ids: &[GeoEntityId],
    ) -> Result<HashMap<GeoEntityId, u64>>;

    fn geo(&self) -> Result<&dyn GeoLookup>;
}
