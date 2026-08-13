use crate::journey_bitmap::{
    BlockKey, JourneyBitmap, TileKey, BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, MAP_WIDTH_OFFSET,
    TILE_WIDTH, TILE_WIDTH_OFFSET,
};
use crate::utils;
use std::collections::HashMap;
const EARTH_RADIUS: f64 = 6371000.0; // unit: meter

const CM2_PER_M2: i64 = 10_000;
const ALL_OFFSET: i16 = BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET;
const BIT_LNG_STEP: f64 = 360.0 / ((1i64 << ALL_OFFSET) as f64);

/// Latitude-corrected area (cm²) of one block with `bit_count` set bits. Shared
/// by `journey_bitmap_area_cm2` and the region index, so the two reconcile.
pub(crate) fn block_area_cm2(tile_key: &TileKey, block_key: &BlockKey, bit_count: u32) -> i64 {
    if bit_count == 0 {
        return 0;
    }
    // Center bit of the block, in bitmap-zoomed tile coordinates.
    let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_key.y as i32
        + BITMAP_WIDTH as i32 * block_key.y() as i32
        + (BITMAP_WIDTH / 2) as i32;
    let bitzoomed_y2 = bitzoomed_y1 + 1;

    let lat1 = utils::tile_y_to_lat(bitzoomed_y1, ALL_OFFSET as i32);
    let lat2 = utils::tile_y_to_lat(bitzoomed_y2, ALL_OFFSET as i32);

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

fn compute_one_tile(journey_bitmap: &JourneyBitmap, tile_key: &TileKey) -> i64 {
    journey_bitmap.peek_tile_without_updating_cache(tile_key, |tile| match tile {
        None => 0,
        Some(tile) => {
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
                    let block_key = BlockKey::from_x_y(0, block_y as u8);
                    block_area_cm2(tile_key, &block_key, bit_count)
                })
                .sum()
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
    journey_bitmap
        .all_tile_keys()
        .map(|tile_key| match tile_area_cache.as_mut() {
            None => compute_one_tile(journey_bitmap, tile_key),
            Some(cache) => *cache
                .entry(*tile_key)
                .or_insert_with(|| compute_one_tile(journey_bitmap, tile_key)),
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
