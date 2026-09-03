use std::collections::HashMap;

use anyhow::Result;

use flutter_rust_bridge::frb;
use geo_data_format::Worldview as GeoWorldview;

pub use crate::achievement::layer::AchievementLayer;
use crate::achievement::region;
pub use crate::achievement::region::{
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

    #[frb(sync, getter)]
    pub fn provenance_path(&self) -> String {
        format!("assets/geo/geo_data_{}.provenance", self.inner().spec().id)
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

pub fn open_installed_geo_data(worldview: Worldview, provenance_hash_hex: String) -> Result<bool> {
    let hash: [u8; 32] = hex::decode(provenance_hash_hex.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("provenance hash must be 32 bytes of hex"))?;
    crate::api::api::get()
        .storage
        .open_installed_geo_data(worldview.into(), hash)
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

// Regions (layered): Flutter Rust Bridge entry points over `achievement::region`.

pub fn region_levels() -> Result<HashMap<RegionKind, LevelSummary>> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| Ok(region::region_levels(store.geo()?)))
}

pub fn region_level_view(
    layer: AchievementLayer,
    level: RegionKind,
    parent: Option<GeoEntityId>,
) -> Result<RegionLevelView> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            let ids = region::level_scope(store.geo()?, level, parent);
            let areas = store.region_areas(layer, &ids)?;
            Ok(region::level_view(store.geo()?, level, &ids, &areas))
        })
}

pub fn region_detail(
    entity_id: GeoEntityId,
    layer: AchievementLayer,
) -> Result<Option<RegionDetail>> {
    crate::api::api::get()
        .storage
        .with_achievement_read(|store| {
            let ids = region::detail_scope(store.geo()?, entity_id);
            let areas = store.region_areas(layer, &ids)?;
            Ok(region::detail_view(store.geo()?, entity_id, &areas))
        })
}
