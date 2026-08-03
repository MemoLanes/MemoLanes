//! Per-entity total area accumulation. Mirrors the trapezoidal-spherical
//! formula used by `compute_one_tile` in
//! `app/rust/src/journey_area_utils.rs:38-46`.

use std::collections::BTreeMap;

use geo_data_format::{
    cell_index, tile_index, GeoEntityId, TileMembership, NO_ENTITY, TILE_GRID_WIDTH,
};

use crate::entities::EntityModel;
use crate::projection::block_area_m2;

const TILE_WIDTH: usize = 128;

pub fn populate_total_areas(
    model: &mut EntityModel,
    tile_lookup: &[TileMembership],
    block_lookup: &BTreeMap<(u16, u16), Vec<u32>>,
) {
    // `block_area_m2(x, y)` is independent of `x`: in the projection `lng` is
    // linear in `x` (so the per-block longitude span is constant) and `lat`
    // depends only on `y`. Precompute one area per grid row once (65_536 evals)
    // instead of re-evaluating sinh/atan/cos for every cell of every tile
    // (~1.8 B evals per worldview). The lookup feeds the same accumulation order, so
    // the result is bit-identical to the per-cell computation.
    let row_area: Vec<f64> = (0..TILE_GRID_WIDTH as i64 * TILE_WIDTH as i64)
        .map(|by| block_area_m2(0, by))
        .collect();

    let slots = model.entities.iter().map(|e| e.id.0).max().unwrap_or(0) as usize + 1;
    let mut parent_of: Vec<Option<GeoEntityId>> = vec![None; slots];
    let mut is_entity: Vec<bool> = vec![false; slots];
    for e in &model.entities {
        parent_of[e.id.0 as usize] = e.parent_id;
        is_entity[e.id.0 as usize] = true;
    }

    let mut acc: Vec<f64> = vec![0.0; slots];
    let credit = |acc: &mut Vec<f64>, leaf: GeoEntityId, area: f64| {
        debug_assert!(
            is_entity.get(leaf.0 as usize).copied().unwrap_or(false),
            "raster leaf {leaf:?} has no entry in the entity model — every id a raster can \
             produce must exist in `model.entities`, or its area silently vanishes"
        );
        let mut cur = Some(leaf);
        while let Some(id) = cur {
            acc[id.0 as usize] += area;
            cur = parent_of[id.0 as usize];
        }
    };

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
                    credit(&mut acc, *id, tile_area);
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
                            let cell = blocks[cell_index(bxo as u8, byo as u8)];
                            if cell != NO_ENTITY {
                                credit(&mut acc, GeoEntityId(cell), row_area[by]);
                            }
                        }
                    }
                }
            }
        }
    }

    for entity in &mut model.entities {
        entity.total_area_m2 = acc[entity.id.0 as usize].round() as u64;
    }
}
