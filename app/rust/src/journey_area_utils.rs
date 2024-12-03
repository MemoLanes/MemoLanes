use crate::journey_bitmap::{
    JourneyBitmap, BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, MAP_WIDTH_OFFSET, TILE_WIDTH,
    TILE_WIDTH_OFFSET,
};
use crate::utils;
/* unit: meter */
pub const EARTH_RADIUS: f64 = 6371000.0;

/* result unit in m^2 */
pub fn get_area_by_journey_bitmap_interation_bit_estimate_block(
    journey_bitmap: &JourneyBitmap,
) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().filter_map(move |(block_pos, block)| {
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
                    let width = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat1.to_radians().cos();
                    /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference of latitudes in radians. */
                    let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();
                    let bit_unit_area = width * height;
                    Some(bit_unit_area * bit_count as f64)
                }
                else {
                    None
                }
            })
        })
        .sum();

    if total_area > 0.0 {
        Some(total_area)
    } else {
        None
    }
}
