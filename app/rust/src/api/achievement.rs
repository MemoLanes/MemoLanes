use std::collections::HashMap;

use anyhow::Result;

use flutter_rust_bridge::frb;
use geo_data_format::Worldview as GeoWorldview;

pub use crate::achievement::layer::AchievementLayer;
use crate::achievement::read_model::region;
pub use crate::achievement::read_model::region::{
    LevelSummary, RegionDetail, RegionEntity, RegionKind, RegionLevelView,
};
pub use geo_data_format::GeoEntityId;

// `GeoEntityId` (external `geo_data_format`) keys `RegionLevelView.entries` /
// `RegionDetail.children`. Mirror its field so Dart gets a value class with
// `==`/`hashCode` (usable as a `Map` key, buildable from an int), not an opaque box.
#[frb(mirror(GeoEntityId))]
pub struct _GeoEntityId(pub u32);

// TODO: Keep this as a Dart-facing wrapper instead of `#[frb(mirror(GeoWorldview))]`.
// FRB mirror types can expose methods already implemented on the original type
// via `#[frb(external)]`, but cannot add bridge-local methods to the mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worldview {
    Iso,
    Chn,
    Usa,
}

impl Worldview {
    #[frb(sync, getter)]
    pub fn asset_path(&self) -> String {
        format!("assets/geo/geo_data_{}.bin", self.inner().spec().id)
    }

    #[frb(sync)]
    pub fn from_id(id: &str) -> Option<Self> {
        GeoWorldview::from_id(id).ok().map(Self::from)
    }

    #[frb(sync, getter)]
    pub fn id(&self) -> String {
        self.inner().spec().id.to_owned()
    }

    #[frb(sync)]
    pub fn default_value() -> Self {
        Self::from(GeoWorldview::ALL[0])
    }

    fn inner(self) -> GeoWorldview {
        match self {
            Self::Iso => GeoWorldview::Iso,
            Self::Chn => GeoWorldview::Chn,
            Self::Usa => GeoWorldview::Usa,
        }
    }
}

impl From<GeoWorldview> for Worldview {
    fn from(worldview: GeoWorldview) -> Self {
        match worldview {
            GeoWorldview::Iso => Self::Iso,
            GeoWorldview::Chn => Self::Chn,
            GeoWorldview::Usa => Self::Usa,
        }
    }
}

impl From<Worldview> for GeoWorldview {
    fn from(worldview: Worldview) -> Self {
        worldview.inner()
    }
}

pub fn init_or_change_geo_data(worldview: Worldview, geo_data: &[u8]) -> Result<()> {
    crate::api::api::get()
        .storage
        .init_or_change_geo_data(worldview.into(), geo_data)
}

/// Explored area for a single layer.
pub fn get_explored_area(layer: AchievementLayer) -> Result<u64> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| store.explored_area_m2(layer))
}

/// Explored area for every layer, read under one snapshot so they can't skew.
pub fn get_explored_area_by_layer() -> Result<HashMap<AchievementLayer, u64>> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            AchievementLayer::ALL_LAYERS
                .into_iter()
                .map(|layer| Ok((layer, store.explored_area_m2(layer)?)))
                .collect()
        })
}

// Regions (layered): Flutter Rust Bridge entry points over `achievement::read_model::region`.

pub fn region_levels() -> Result<HashMap<RegionKind, LevelSummary>> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            Ok(store.geo().map_or_else(HashMap::new, region::region_levels))
        })
}

pub fn region_level_view(
    layer: AchievementLayer,
    level: RegionKind,
    parent: Option<GeoEntityId>,
) -> Result<RegionLevelView> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            let Some(geo) = store.geo() else {
                return Ok(RegionLevelView {
                    level,
                    visited_count: 0,
                    region_count: 0,
                    entries: HashMap::new(),
                });
            };
            Ok(region::region_level_view(
                &store.region_states(&[layer])?,
                geo,
                layer,
                level,
                parent,
            ))
        })
}

pub fn region_detail(
    entity_id: GeoEntityId,
    layer: AchievementLayer,
) -> Result<Option<RegionDetail>> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            let Some(geo) = store.geo() else {
                return Ok(None);
            };
            Ok(region::region_detail(
                &store.region_states(&[layer])?,
                geo,
                entity_id,
                layer,
            ))
        })
}
