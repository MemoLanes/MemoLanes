//! Layered region read-model: region coverage state joined with the worldview geo
//! tree into list / detail shapes. Pure functions over `(states, geo)`; the
//! `api/achievement` Flutter Rust Bridge functions load the state and wrap.
//! Every per-region read is scoped to one `AchievementLayer` (the hot path).

use std::collections::HashMap;

use geo_data_format::{GeoEntityId, GeoEntityKind};

use crate::achievement::compute::region_state::RegionStateMap;
use crate::achievement::layer::AchievementLayer;
use crate::geo::GeoLookup;

/// Geo admin level (mirrors `GeoEntityKind` for the wire). `Hash` so it can key
/// the `region_levels` map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    Continent,
    Country,
    Province,
    City,
}

impl From<GeoEntityKind> for RegionKind {
    fn from(k: GeoEntityKind) -> Self {
        match k {
            GeoEntityKind::Continent => RegionKind::Continent,
            GeoEntityKind::Country => RegionKind::Country,
            GeoEntityKind::Province => RegionKind::Province,
            GeoEntityKind::City => RegionKind::City,
        }
    }
}

impl RegionKind {
    fn to_geo(self) -> GeoEntityKind {
        match self {
            RegionKind::Continent => GeoEntityKind::Continent,
            RegionKind::Country => GeoEntityKind::Country,
            RegionKind::Province => GeoEntityKind::Province,
            RegionKind::City => GeoEntityKind::City,
        }
    }
}

/// One entity's coverage within the queried layer. Unvisited → zeros. Used by
/// both the level list and the detail view; the `entity_id` lives in the map
/// key (`RegionLevelView.entries` / `RegionDetail.children`). The coverage
/// fraction is `visited_area_m2 / total_area_m2`, left to the frontend.
pub struct RegionEntity {
    pub kind: RegionKind,
    pub name_key: String,
    pub visited_area_m2: u64,
    pub total_area_m2: u64,
}

/// Per-level rollup for the level picker. Layer-independent (geo-only count).
pub struct LevelSummary {
    pub region_count: u32,
}

/// One level under a parent, in one layer: headline counts plus the full entity
/// map (visited + unvisited — drives list + grey-out). A level is complete when
/// `visited_count == region_count > 0`.
pub struct RegionLevelView {
    pub level: RegionKind,
    pub visited_count: u32,
    pub region_count: u32,
    pub entries: HashMap<GeoEntityId, RegionEntity>,
}

/// One entity (in the queried layer) plus its direct children, keyed by id.
pub struct RegionDetail {
    pub entity_id: GeoEntityId,
    pub node: RegionEntity,
    pub children: HashMap<GeoEntityId, RegionEntity>,
}

fn entities_in_scope(
    geo: &dyn GeoLookup,
    level: GeoEntityKind,
    parent: Option<GeoEntityId>,
) -> Vec<GeoEntityId> {
    let ids = geo.entities_of_kind(level);
    match parent {
        None => ids.to_vec(),
        Some(parent) => ids
            .iter()
            .copied()
            .filter(|&id| geo.entity(id).map(|e| e.parent_id) == Some(Some(parent)))
            .collect(),
    }
}

/// One entity's coverage in `layer`, or `None` if the id is unknown.
fn region_entity(
    states: &RegionStateMap,
    geo: &dyn GeoLookup,
    id: GeoEntityId,
    layer: AchievementLayer,
) -> Option<RegionEntity> {
    let entity = geo.entity(id)?;
    let visited_area_m2 = states.get(&(layer, id)).map_or(0, |s| s.visited_area_m2);
    Some(RegionEntity {
        kind: entity.kind.into(),
        name_key: entity.name_key.clone(),
        visited_area_m2,
        total_area_m2: entity.total_area_m2,
    })
}

/// Levels present in the current map (layer-independent), keyed by kind.
pub fn region_levels(geo: &dyn GeoLookup) -> HashMap<RegionKind, LevelSummary> {
    [
        GeoEntityKind::Continent,
        GeoEntityKind::Country,
        GeoEntityKind::Province,
        GeoEntityKind::City,
    ]
    .into_iter()
    .filter_map(|kind| {
        let region_count = geo.entities_of_kind(kind).len() as u32;
        (region_count > 0).then_some((kind.into(), LevelSummary { region_count }))
    })
    .collect()
}

/// Joined view for `level` within `parent`, in one layer. See [`RegionLevelView`].
pub fn region_level_view(
    states: &RegionStateMap,
    geo: &dyn GeoLookup,
    layer: AchievementLayer,
    level: RegionKind,
    parent: Option<GeoEntityId>,
) -> RegionLevelView {
    let mut entries = HashMap::new();
    let mut visited_count = 0u32;

    for id in entities_in_scope(geo, level.to_geo(), parent) {
        let Some(entity) = geo.entity(id) else {
            continue;
        };
        let visited_area_m2 = match states.get(&(layer, id)) {
            Some(s) => {
                visited_count += 1;
                s.visited_area_m2
            }
            None => 0,
        };
        entries.insert(
            id,
            RegionEntity {
                kind: entity.kind.into(),
                name_key: entity.name_key.clone(),
                visited_area_m2,
                total_area_m2: entity.total_area_m2,
            },
        );
    }

    let region_count = entries.len() as u32;
    RegionLevelView {
        level,
        visited_count,
        region_count,
        entries,
    }
}

/// One entity's coverage in `layer` plus its direct children, keyed by id.
pub fn region_detail(
    states: &RegionStateMap,
    geo: &dyn GeoLookup,
    entity_id: GeoEntityId,
    layer: AchievementLayer,
) -> Option<RegionDetail> {
    let node = region_entity(states, geo, entity_id, layer)?;
    let children = geo
        .children(entity_id)
        .iter()
        .filter_map(|&child| region_entity(states, geo, child, layer).map(|e| (child, e)))
        .collect();
    Some(RegionDetail {
        entity_id,
        node,
        children,
    })
}
