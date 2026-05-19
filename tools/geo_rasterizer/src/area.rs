//! Per-entity total area accumulation. Mirrors the trapezoidal-spherical
//! formula used by `compute_one_tile` in
//! `app/rust/src/journey_area_utils.rs:38-46`.

use std::collections::BTreeMap;

use geo_data_format::{GeoEntityId, GeoEntityKind, TileMembership};

use crate::entities::EntityModel;
use crate::projection::block_area_m2;

const MAP_WIDTH: usize = 512;
const TILE_WIDTH: usize = 128;

pub fn populate_total_areas(
    model: &mut EntityModel,
    tile_lookup: &[TileMembership],
    block_lookup: &BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>>,
) {
    let mut country_areas: BTreeMap<GeoEntityId, f64> = BTreeMap::new();
    for ty in 0..MAP_WIDTH {
        for tx in 0..MAP_WIDTH {
            let tile_idx = ty * MAP_WIDTH + tx;
            match &tile_lookup[tile_idx] {
                TileMembership::None => {}
                TileMembership::Single(id) => {
                    let mut tile_area = 0.0;
                    for byo in 0..TILE_WIDTH {
                        for bxo in 0..TILE_WIDTH {
                            let bx = tx as i64 * TILE_WIDTH as i64 + bxo as i64;
                            let by = ty as i64 * TILE_WIDTH as i64 + byo as i64;
                            tile_area += block_area_m2(bx, by);
                        }
                    }
                    *country_areas.entry(*id).or_default() += tile_area;
                }
                TileMembership::Border => {
                    let blocks = match block_lookup.get(&(tx as u16, ty as u16)) {
                        Some(b) => b,
                        None => continue,
                    };
                    for byo in 0..TILE_WIDTH {
                        for bxo in 0..TILE_WIDTH {
                            let cell = &blocks[byo * TILE_WIDTH + bxo];
                            if let Some(id) = cell {
                                let bx = tx as i64 * TILE_WIDTH as i64 + bxo as i64;
                                let by = ty as i64 * TILE_WIDTH as i64 + byo as i64;
                                *country_areas.entry(*id).or_default() += block_area_m2(bx, by);
                            }
                        }
                    }
                }
            }
        }
    }
    let mut continent_areas: BTreeMap<GeoEntityId, u64> = BTreeMap::new();
    for entity in &mut model.entities {
        if matches!(entity.kind, GeoEntityKind::Country) {
            let area = country_areas.get(&entity.id).copied().unwrap_or(0.0);
            entity.total_area_m2 = area.round() as u64;
            if let Some(parent) = entity.parent_id {
                *continent_areas.entry(parent).or_default() += entity.total_area_m2;
            }
        }
    }
    for entity in &mut model.entities {
        if matches!(entity.kind, GeoEntityKind::Continent) {
            entity.total_area_m2 = continent_areas.get(&entity.id).copied().unwrap_or(0);
        }
    }
}
