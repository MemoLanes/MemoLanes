use crate::geo::GeoLookup;
use geo_data_format::{GeoEntityId, GeoEntityKind};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionKind {
    Continent,
    Admin0,
    Admin1,
    Admin2,
}

impl From<GeoEntityKind> for RegionKind {
    fn from(k: GeoEntityKind) -> Self {
        match k {
            GeoEntityKind::Continent => RegionKind::Continent,
            GeoEntityKind::Admin0 => RegionKind::Admin0,
            GeoEntityKind::Admin1 => RegionKind::Admin1,
            GeoEntityKind::Admin2 => RegionKind::Admin2,
        }
    }
}

impl RegionKind {
    fn to_geo(self) -> GeoEntityKind {
        match self {
            RegionKind::Continent => GeoEntityKind::Continent,
            RegionKind::Admin0 => GeoEntityKind::Admin0,
            RegionKind::Admin1 => GeoEntityKind::Admin1,
            RegionKind::Admin2 => GeoEntityKind::Admin2,
        }
    }
}

pub struct RegionNameKey {
    pub value: String,
}

pub struct RegionEntity {
    pub kind: RegionKind,
    pub name_key: RegionNameKey,
    pub iso_a3_eh: Option<String>,
    pub visited_area_m2: u64,
    pub total_area_m2: u64,
}

pub struct LevelSummary {
    pub region_count: u32,
}

pub struct RegionLevelView {
    pub level: RegionKind,
    pub visited_count: u32,
    pub region_count: u32,
    pub entries: HashMap<GeoEntityId, RegionEntity>,
}

pub struct RegionDetail {
    pub entity_id: GeoEntityId,
    pub node: RegionEntity,
    pub children: HashMap<GeoEntityId, RegionEntity>,
}

pub fn level_scope(
    geo: &dyn GeoLookup,
    level: RegionKind,
    parent: Option<GeoEntityId>,
) -> Vec<GeoEntityId> {
    let ids = geo.entities_of_kind(level.to_geo());
    match parent {
        None => ids.to_vec(),
        Some(parent) => ids
            .iter()
            .copied()
            .filter(|&id| geo.entity(id).map(|e| e.parent_id) == Some(Some(parent)))
            .collect(),
    }
}

pub fn detail_scope(geo: &dyn GeoLookup, entity_id: GeoEntityId) -> Vec<GeoEntityId> {
    let mut ids = vec![entity_id];
    ids.extend_from_slice(geo.children(entity_id));
    ids
}

fn region_entity(
    geo: &dyn GeoLookup,
    id: GeoEntityId,
    areas: &HashMap<GeoEntityId, u64>,
) -> Option<RegionEntity> {
    let entity = geo.entity(id)?;
    Some(RegionEntity {
        kind: entity.kind.into(),
        name_key: RegionNameKey {
            value: entity.name_key.clone(),
        },
        iso_a3_eh: entity.iso_a3_eh.clone(),
        visited_area_m2: areas.get(&id).copied().unwrap_or(0),
        total_area_m2: entity.total_area_m2,
    })
}

pub fn region_levels(geo: &dyn GeoLookup) -> HashMap<RegionKind, LevelSummary> {
    [
        GeoEntityKind::Continent,
        GeoEntityKind::Admin0,
        GeoEntityKind::Admin1,
        GeoEntityKind::Admin2,
    ]
    .into_iter()
    .filter_map(|kind| {
        let region_count = geo.entities_of_kind(kind).len() as u32;
        (region_count > 0).then_some((kind.into(), LevelSummary { region_count }))
    })
    .collect()
}

pub fn level_view(
    geo: &dyn GeoLookup,
    level: RegionKind,
    ids: &[GeoEntityId],
    areas: &HashMap<GeoEntityId, u64>,
) -> RegionLevelView {
    let mut entries = HashMap::new();
    let mut visited_count = 0u32;
    for &id in ids {
        let Some(entity) = region_entity(geo, id, areas) else {
            continue;
        };
        if areas.contains_key(&id) {
            visited_count += 1;
        }
        entries.insert(id, entity);
    }
    let region_count = entries.len() as u32;
    RegionLevelView {
        level,
        visited_count,
        region_count,
        entries,
    }
}

pub fn detail_view(
    geo: &dyn GeoLookup,
    entity_id: GeoEntityId,
    areas: &HashMap<GeoEntityId, u64>,
) -> Option<RegionDetail> {
    let node = region_entity(geo, entity_id, areas)?;
    let children = geo
        .children(entity_id)
        .iter()
        .filter_map(|&child| region_entity(geo, child, areas).map(|e| (child, e)))
        .collect();
    Some(RegionDetail {
        entity_id,
        node,
        children,
    })
}
