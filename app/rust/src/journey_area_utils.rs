use crate::journey_bitmap::{
    JourneyBitmap, BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, MAP_WIDTH_OFFSET, TILE_WIDTH,
    TILE_WIDTH_OFFSET,
};
use crate::utils;
//use ordered_float::OrderedFloat;
const EARTH_RADIUS: f64 = 6371000.0; // unit: meter

// Result unit in m^2. This area calculating method by using center bit in a
// block has better efficiency and accuracy compared to simple interation and other methods.
// codes for different calculating methods can be found here:
// https://github.com/TimRen01/TimRen01_repo/tree/compare_method_calculate_area_by_journey
pub fn compute_journey_bitmap_area(journey_bitmap: &mut JourneyBitmap) -> u64 {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter_mut()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter_mut().filter_map(move |(block_pos, block)| {
                let bit_count = block.count();
                if bit_count > 0 {
                    // calculate center bit in each block for bit_unit_area
                    // Calculate the top-left coordinates of this bitmap point
                    let bitzoomed_x1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.0 as i32
                    + BITMAP_WIDTH as i32 * block_pos.0 as i32
                    + (BITMAP_WIDTH/2) as i32;
                    let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.1 as i32
                    + BITMAP_WIDTH as i32 * block_pos.1 as i32
                    + (BITMAP_WIDTH/2) as i32;

                    // Bottom-right coordinates (add one bit length to each side)
                    let bitzoomed_x2 = bitzoomed_x1 + 1;
                    let bitzoomed_y2 = bitzoomed_y1 + 1;

                    // Convert these to latitude/longitude
                    let (lng1, lat1) = utils::tile_x_y_to_lng_lat(
                        bitzoomed_x1,
                        bitzoomed_y1,
                        (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                    );
                    let (lng2, lat2) = utils::tile_x_y_to_lng_lat(
                        bitzoomed_x2,
                        bitzoomed_y2,
                        (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                    );

                    /* formula derived from spherical geometry of Earth */
                    /* width=R⋅Δλ⋅cos(ϕ), where Δλ = λ2-λ1 is the difference of longitudes in radians, ϕ is the latitude in radians*/
                    let width_top = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat1.to_radians().cos();
                    let width_bottom = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat2.to_radians().cos();
                    let avg_width = (width_top + width_bottom) / 2.0;
                    /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference of latitudes in radians. */
                    let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();
                    let bit_unit_area = avg_width * height;
                    if block.bit_unit_area.is_none() {
                        block.bit_unit_area = Some(bit_unit_area as u64);
                    }
                    Some(bit_unit_area * bit_count as f64)
                }
                else {
                    None
                }
            })
        })
        .sum();
    // we don't have that much precision in the journey bitmap anyway
    total_area.round() as u64
}

pub fn compute_journey_bitmap_area_incremented(journey_bitmap: &mut JourneyBitmap) -> u64 {
    journey_bitmap
        .tiles
        .values_mut()
        .filter(|tile| tile.update_flag)
        .map(|tile| {
            let tile_inc_area: u64 = tile
                .blocks
                .values_mut()
                .filter(|block| block.update_flag)
                .filter_map(|block| {
                    let bit_inc = block.count_inc;
                    let unit_bit_area = block.bit_unit_area.unwrap();
                    if bit_inc == 0 {
                        return None;
                    }
                    block.update_flag = false;
                    block.count_inc = 0;
                    Some(unit_bit_area * bit_inc)
                })
                .sum();
            tile.update_flag = false;
            tile_inc_area
        })
        .sum()
}
