//! Total region areas over the same classified cells used by attribution.
//! Mercator accumulation retains its historical order/formula. Polar areas
//! use the journey grid's equal-area pixels, clipped and quantized at the seam.

use crate::{entities::EntityModel, projection::block_area_m2};
use bitmap_geometry::{block_pixel_bounds, polar_rectangle_area_cm2, BitmapPlane, BITMAP_WIDTH};
use geo_data_format::{cell_index, GeoEntityId, PlaneRaster, TileMembership, NO_ENTITY};
use std::collections::BTreeMap;

const TILE_WIDTH: usize = 128;

pub fn populate_total_areas(
    model: &mut EntityModel,
    tiles: &[TileMembership],
    blocks: &BTreeMap<(u16, u16), Vec<u32>>,
) {
    populate_total_areas_planes(
        model,
        &[PlaneRaster {
            plane: BitmapPlane::Mercator,
            tiles,
            blocks,
        }],
    );
}

pub fn populate_total_areas_planes(model: &mut EntityModel, rasters: &[PlaneRaster<'_>]) {
    let row_area: Vec<f64> = (0..65536).map(|by| block_area_m2(0, by)).collect();
    let slots = model.entities.iter().map(|e| e.id.0).max().unwrap_or(0) as usize + 1;
    let mut parent_of = vec![None; slots];
    let mut is_entity = vec![false; slots];
    for e in &model.entities {
        parent_of[e.id.0 as usize] = e.parent_id;
        is_entity[e.id.0 as usize] = true;
    }
    let mut acc = vec![0.0; slots];
    let mut polar_acc_cm2 = vec![0_i64; slots];
    let mut credit = |leaf: GeoEntityId, mercator_m2: f64, polar_cm2: i64| {
        assert!(
            is_entity.get(leaf.0 as usize).copied().unwrap_or(false),
            "raster leaf {leaf:?} has no entity"
        );
        let mut cur = Some(leaf);
        while let Some(id) = cur {
            acc[id.0 as usize] += mercator_m2;
            polar_acc_cm2[id.0 as usize] += polar_cm2;
            cur = parent_of[id.0 as usize];
        }
    };
    for raster in rasters {
        let width = raster.plane.grid_width() as usize;
        assert_eq!(raster.tiles.len(), width * width);
        for tx in 0..width {
            for ty in 0..width {
                match raster.tiles[tx * width + ty] {
                    TileMembership::None => {}
                    TileMembership::Single(id) => {
                        let (mercator_m2, polar_cm2) = if raster.plane == BitmapPlane::Mercator {
                            let mut area = 0.0;
                            for byo in 0..TILE_WIDTH {
                                for _ in 0..TILE_WIDTH {
                                    area += row_area[ty * TILE_WIDTH + byo];
                                }
                            }
                            (area, 0)
                        } else {
                            let side = TILE_WIDTH as i64 * BITMAP_WIDTH;
                            let x = tx as i64 * side;
                            let y = ty as i64 * side;
                            (0.0, polar_rectangle_area_cm2(x..x + side, y..y + side))
                        };
                        credit(id, mercator_m2, polar_cm2);
                    }
                    TileMembership::Border => {
                        let cells = raster
                            .blocks
                            .get(&(tx as u16, ty as u16))
                            .expect("missing border cells");
                        for byo in 0..TILE_WIDTH {
                            for bxo in 0..TILE_WIDTH {
                                let id = cells[cell_index(bxo as u8, byo as u8)];
                                if id == NO_ENTITY {
                                    continue;
                                }
                                let (mercator_m2, polar_cm2) =
                                    if raster.plane == BitmapPlane::Mercator {
                                        (row_area[ty * TILE_WIDTH + byo], 0)
                                    } else {
                                        let (x, y) = block_pixel_bounds(
                                            (tx as u16, ty as u16),
                                            (bxo as u8, byo as u8),
                                        );
                                        (0.0, polar_rectangle_area_cm2(x, y))
                                    };
                                credit(GeoEntityId(id), mercator_m2, polar_cm2);
                            }
                        }
                    }
                }
            }
        }
    }
    for entity in &mut model.entities {
        let index = entity.id.0 as usize;
        entity.total_area_m2 = (acc[index] + polar_acc_cm2[index] as f64 / 10_000.0).round() as u64;
    }
}
