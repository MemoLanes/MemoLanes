use std::collections::HashMap;

use anyhow::Result;

use flutter_rust_bridge::frb;
use geo_data_format::{all_worldviews, WorldviewVariant};

pub use crate::achievement::layer::AchievementLayer;
use crate::achievement::read_model::region;
pub use crate::achievement::read_model::region::{
    LevelSummary, RegionDetail, RegionEntity, RegionKind, RegionLevelView,
};
pub use geo_data_format::Worldview;

// `Worldview` lives in `geo_data_format` (external crate), so FRB can't see its
// fields to translate it by value. Mirror the field list here so the Dart side
// gets a plain class with `id`/`name_key`/`description_key`, not an opaque box.
#[frb(mirror(Worldview))]
pub struct _Worldview {
    pub id: String,
    pub name_key: String,
    pub description_key: String,
}

// Geo (worldview asset): the prerequisite for every region read.

/// What `get_geo` reports: the user's selected worldview and the offered list.
pub struct GeoStatus {
    /// The user's chosen worldview id (a [`Worldview::id`]), persisted; defaults
    /// to the first [`WorldviewVariant`] ("iso") until the user picks another.
    pub selected_worldview: String,
    /// The full offered worldview list (the compiled set).
    pub worldviews: Vec<Worldview>,
}

/// Install (or switch to) a worldview. `worldview` is a [`Worldview::id`] (e.g.
/// `"iso"`); the backend reads the corresponding bundled `geo_data_<id>.bin`
/// from its configured geo dir. A worldview change re-derives the region index.
/// Errors on an unknown `worldview` id or a missing asset file.
pub fn set_geo(worldview: String) -> Result<()> {
    let worldview = WorldviewVariant::from_id(&worldview)?;
    crate::api::api::get().storage.set_geo(worldview)
}

/// The user's selected worldview id (persisted, default "iso") and the full
/// offered worldview list ([`all_worldviews`], the compiled set).
pub fn get_geo() -> Result<GeoStatus> {
    let storage = &crate::api::api::get().storage;
    Ok(GeoStatus {
        selected_worldview: storage.selected_worldview().spec().id.to_string(),
        worldviews: all_worldviews(),
    })
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
    parent: Option<u32>,
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

pub fn region_detail(entity_id: u32, layer: AchievementLayer) -> Result<Option<RegionDetail>> {
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
