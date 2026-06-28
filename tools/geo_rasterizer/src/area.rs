//! Per-entity total area accumulation. Mirrors the trapezoidal-spherical
//! formula used by `compute_one_tile` in
//! `app/rust/src/journey_area_utils.rs:38-46`.

use std::collections::BTreeMap;

use geo_data_format::{
    cell_index, tile_index, GeoEntityId, GeoEntityKind, TileMembership, TILE_GRID_WIDTH,
};

use crate::entities::EntityModel;
use crate::projection::block_area_m2;

const TILE_WIDTH: usize = 128;

pub fn populate_total_areas(
    model: &mut EntityModel,
    tile_lookup: &[TileMembership],
    block_lookup: &BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>>,
) {
    // `block_area_m2(x, y)` is independent of `x`: in the projection `lng` is
    // linear in `x` (so the per-block longitude span is constant) and `lat`
    // depends only on `y`. Precompute one area per grid row once (65_536 evals)
    // instead of re-evaluating sinh/atan/cos for every cell of every tile
    // (~1.8 B evals per POV). The lookup feeds the same accumulation order, so
    // the result is bit-identical to the per-cell computation.
    let row_area: Vec<f64> = (0..TILE_GRID_WIDTH as i64 * TILE_WIDTH as i64)
        .map(|by| block_area_m2(0, by))
        .collect();

    let mut country_areas: BTreeMap<GeoEntityId, f64> = BTreeMap::new();
    for tx in 0..TILE_GRID_WIDTH {
        for ty in 0..TILE_GRID_WIDTH {
            let tile_idx = tile_index(tx as u16, ty as u16);
            match &tile_lookup[tile_idx] {
                TileMembership::None => {}
                TileMembership::Single(id) => {
                    let mut tile_area = 0.0;
                    for byo in 0..TILE_WIDTH {
                        let by = ty * TILE_WIDTH + byo;
                        for _bxo in 0..TILE_WIDTH {
                            tile_area += row_area[by];
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
                        let by = ty * TILE_WIDTH + byo;
                        for bxo in 0..TILE_WIDTH {
                            // Weight by row `byo`, so index the cell at (x=bxo, y=byo).
                            let cell = &blocks[cell_index(bxo as u8, byo as u8)];
                            if let Some(id) = cell {
                                *country_areas.entry(*id).or_default() += row_area[by];
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
