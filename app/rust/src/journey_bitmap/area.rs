use super::geometry::{
    block_pixel_bounds, polar_pixel_size_meters, polar_rect_radius_bounds_squared, BITMAP_WIDTH,
    MERCATOR_WORLD_PIXEL_OFFSET, POLAR_DISK_RADIUS, TILE_WIDTH,
};
use super::{Block, JourneyBitmap, TileKey};
use crate::spherical_geometry::EARTH_RADIUS_METERS as EARTH_RADIUS;
use crate::utils;
use bitvec::{order::Msb0, view::BitView};
use std::collections::HashMap;
use std::ops::Range;

const CM2_PER_M2: i64 = 10_000;
const BIT_LNG_STEP: f64 = 360.0 / ((1i64 << MERCATOR_WORLD_PIXEL_OFFSET) as f64);

fn polar_bit_area_cm2() -> i64 {
    let pixel_side_m = polar_pixel_size_meters();
    (pixel_side_m * pixel_side_m * CM2_PER_M2 as f64).round() as i64
}

/// Fraction of a native pixel cell [x,x+1] × [y,y+1] inside the LAEA cap disk.
fn polar_bit_fraction(x: i64, y: i64) -> f64 {
    let (near, far) = polar_rect_radius_bounds_squared(x..x + 1, y..y + 1);
    let radius_squared = POLAR_DISK_RADIUS * POLAR_DISK_RADIUS;
    if near >= radius_squared {
        return 0.0;
    }
    if far <= radius_squared {
        return 1.0;
    }

    let x = x - POLAR_DISK_RADIUS;
    let y = y - POLAR_DISK_RADIUS;
    // Reflect into the first octant. Integrating along the smaller coordinate
    // keeps the circle's slope at most ~1, including pixels near the axes.
    let x = x.abs().min((x + 1).abs()) as f64;
    let y = y.abs().min((y + 1).abs()) as f64;
    let (x, y) = (x.min(y), x.max(y));
    let radius = POLAR_DISK_RADIUS as f64;
    let circle = |t: f64| ((radius - t) * (radius + t)).max(0.0).sqrt();
    let full_end = circle(y + 1.0).clamp(x, x + 1.0);
    let arc_end = circle(y).clamp(full_end, x + 1.0);
    let height = |t| circle(t) - y;
    // Simpson integration over at most one native pixel. At radius 2^19 its
    // error is far below the 1 cm² quantization, without subtracting huge sectors.
    let curved_area = (arc_end - full_end)
        * (height(full_end) + 4.0 * height((full_end + arc_end) / 2.0) + height(arc_end))
        / 6.0;
    (full_end - x + curved_area).clamp(0.0, 1.0)
}

fn polar_block_area_cm2(x: Range<i64>, y: Range<i64>, block: &Block, bit_area_cm2: i64) -> i64 {
    let (near, far) = polar_rect_radius_bounds_squared(x.clone(), y.clone());
    let radius_squared = POLAR_DISK_RADIUS * POLAR_DISK_RADIUS;
    if near >= radius_squared {
        return 0;
    }
    if far <= radius_squared {
        return bit_area_cm2 * block.count() as i64;
    }
    block
        .raw_data()
        .view_bits::<Msb0>()
        .iter_ones()
        .map(|i| {
            let x = x.start + (i % BITMAP_WIDTH as usize) as i64;
            let y = y.start + (i / BITMAP_WIDTH as usize) as i64;
            // Quantize each bit independently so delta accumulation stays exactly
            // additive even when several journeys fill the same boundary block.
            (polar_bit_fraction(x, y) * bit_area_cm2 as f64).round() as i64
        })
        .sum()
}

/// Latitude-corrected Mercator block area (cm²). Shared by bitmap area and the
/// region index, so the two reconcile. Callers match the tile projection first.
pub(crate) fn mercator_block_area_cm2(tile_y: u16, block_y: u8, bit_count: u32) -> i64 {
    if bit_count == 0 {
        return 0;
    }
    // Center bit of the block, in bitmap-zoomed tile coordinates.
    let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_y as i32
        + BITMAP_WIDTH as i32 * block_y as i32
        + (BITMAP_WIDTH / 2) as i32;
    let bitzoomed_y2 = bitzoomed_y1 + 1;

    let lat1 = utils::tile_y_to_lat(bitzoomed_y1, MERCATOR_WORLD_PIXEL_OFFSET as i32);
    let lat2 = utils::tile_y_to_lat(bitzoomed_y2, MERCATOR_WORLD_PIXEL_OFFSET as i32);

    /* formula derived from spherical geometry of Earth */
    /* width=R⋅Δλ⋅cos(ϕ), where Δλ is the difference of longitudes in radians, ϕ is the latitude in radians*/
    let width_top = EARTH_RADIUS * BIT_LNG_STEP.to_radians() * lat1.to_radians().cos();
    let width_bottom = EARTH_RADIUS * BIT_LNG_STEP.to_radians() * lat2.to_radians().cos();
    let avg_width = (width_top + width_bottom) / 2.0;
    /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference of latitudes in radians. */
    let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();

    let bit_area_cm2 = (avg_width * height * CM2_PER_M2 as f64).round() as i64;
    bit_area_cm2 * bit_count as i64
}

fn compute_tile(bitmap: &JourneyBitmap, key: &TileKey) -> i64 {
    bitmap.peek_tile_without_updating_cache(key, |tile| {
        let Some(tile) = tile else {
            return 0;
        };
        match *key {
            TileKey::Mercator { y, .. } => {
                // A bit's area depends only on its latitude row. Aggregate all
                // blocks at the same y before doing the comparatively expensive
                // Mercator and trigonometric work.
                let mut bit_counts_by_row = [0_u32; TILE_WIDTH as usize];
                for (block_key, block) in tile.iter() {
                    bit_counts_by_row[block_key.y() as usize] += block.count();
                }

                bit_counts_by_row
                    .into_iter()
                    .enumerate()
                    .filter(|(_, bit_count)| *bit_count != 0)
                    .map(|(block_y, bit_count)| {
                        mercator_block_area_cm2(y, block_y as u8, bit_count)
                    })
                    .sum()
            }
            TileKey::NorthPolar { x, y } | TileKey::SouthPolar { x, y } => {
                let bit_area_cm2 = polar_bit_area_cm2();
                tile.iter()
                    .map(|(block_key, block)| {
                        let (x, y) = block_pixel_bounds((x, y), (block_key.x(), block_key.y()));
                        polar_block_area_cm2(x, y, block, bit_area_cm2)
                    })
                    .sum()
            }
        }
    })
}

// Result unit in cm^2. This area calculating method by using center bit in a
// block has better efficiency and accuracy compared to simple interation and other methods.
// codes for different calculating methods can be found here:
// https://github.com/TimRen01/TimRen01_repo/tree/compare_method_calculate_area_by_journey
pub fn journey_bitmap_area_cm2(
    journey_bitmap: &JourneyBitmap,
    mut tile_area_cache: Option<&mut HashMap<TileKey, i64>>,
) -> i64 {
    // Each key identifies its projection; every plane uses the same cache.
    journey_bitmap
        .all_tile_keys()
        .map(|key| match tile_area_cache.as_mut() {
            None => compute_tile(journey_bitmap, key),
            Some(cache) => *cache
                .entry(*key)
                .or_insert_with(|| compute_tile(journey_bitmap, key)),
        })
        .sum()
}

/// Half-up, clamped at zero — a negative only reaches here from a corrupted or
/// tampered cache.db, and must read as 0 rather than wrap.
pub fn cm2_to_m2_rounded(area_cm2: i64) -> u64 {
    (area_cm2.max(0) as u64 + (CM2_PER_M2 / 2) as u64) / CM2_PER_M2 as u64
}

pub fn journey_bitmap_area_m2_rounded(
    journey_bitmap: &JourneyBitmap,
    tile_area_cache: Option<&mut HashMap<TileKey, i64>>,
) -> u64 {
    cm2_to_m2_rounded(journey_bitmap_area_cm2(journey_bitmap, tile_area_cache))
}
