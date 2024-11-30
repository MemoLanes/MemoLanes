use crate::journey_bitmap::{
    JourneyBitmap, BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, MAP_WIDTH_OFFSET, TILE_WIDTH,
    TILE_WIDTH_OFFSET,
};
use crate::utils;
/* unit: meter */
pub const EARTH_RADIUS: f64 = 6371000.0;

pub fn get_area_by_journey_bitmap_interation_bit(journey_bitmap: &JourneyBitmap) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().flat_map(move |(block_pos, block)| {
                (0..BITMAP_WIDTH).flat_map(move |bitmap_y| {
                    (0..BITMAP_WIDTH).filter_map(move |bitmap_x| {
                        if block.is_visited(bitmap_x as u8, bitmap_y as u8) {
                            // Calculate the top-left coordinates of this bitmap point
                            let bitzoomed_x1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.0 as i32
                            + BITMAP_WIDTH as i32 * block_pos.0 as i32
                            + bitmap_x as i32;
                            let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.1 as i32
                            + BITMAP_WIDTH as i32 * block_pos.1 as i32
                            + bitmap_y as i32;

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
                            /* width=R⋅Δλ⋅cos(ϕ), where Δλ = λ2-λ1 is the difference in longitudes in radians, ϕ is the latitude in radians*/
                            let width = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat1.to_radians().cos();
                             /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference in latitudes in radians. */
                            let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();
                            Some(width * height)
                        } else {
                            None
                        }
                    })
                })
            })
        })
        .sum();

    if total_area > 0.0 {
        Some(total_area)
    } else {
        None
    }
}

pub fn get_area_by_journey_bitmap_interation_bit_width_only(
    journey_bitmap: &JourneyBitmap,
) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().flat_map(move |(block_pos, block)| {
                (0..BITMAP_WIDTH).flat_map(move |bitmap_y| {
                    (0..BITMAP_WIDTH).filter_map(move |bitmap_x| {
                        if block.is_visited(bitmap_x as u8, bitmap_y as u8) {
                            // Calculate the top-left coordinates of this bitmap point
                            let bitzoomed_x1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.0 as i32
                            + BITMAP_WIDTH as i32 * block_pos.0 as i32
                            + bitmap_x as i32;
                            let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.1 as i32
                            + BITMAP_WIDTH as i32 * block_pos.1 as i32
                            + bitmap_y as i32;

                            // Bottom-right coordinates (add one bit length to each side)
                            let bitzoomed_x2 = bitzoomed_x1 + 1;
                            let bitzoomed_y2 = bitzoomed_y1 + 1;

                            // Convert these to latitude/longitude
                            let (lng1, lat1) = utils::tile_x_y_to_lng_lat(
                                bitzoomed_x1,
                                bitzoomed_y1,
                                (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                            );
                            let (lng2, _lat2) = utils::tile_x_y_to_lng_lat(
                                bitzoomed_x2,
                                bitzoomed_y2,
                                (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                            );

                            /* formula derived from spherical geometry of Earth */
                            /* width=R⋅Δλ⋅cos(ϕ), where Δλ = λ2-λ1 is the difference in longitudes in radians, ϕ is the latitude in radians*/
                            let width = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat1.to_radians().cos();
                             /* use width as height for approximation. */
                            Some(width * width)
                        } else {
                            None
                        }
                    })
                })
            })
        })
        .sum();

    if total_area > 0.0 {
        Some(total_area)
    } else {
        None
    }
}

pub fn get_area_by_journey_bitmap_interation_bit_height_only(
    journey_bitmap: &JourneyBitmap,
) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().flat_map(move |(block_pos, block)| {
                (0..BITMAP_WIDTH).flat_map(move |bitmap_y| {
                    (0..BITMAP_WIDTH).filter_map(move |bitmap_x| {
                        if block.is_visited(bitmap_x as u8, bitmap_y as u8) {
                            // Calculate the top-left coordinates of this bitmap point
                            let bitzoomed_x1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.0 as i32
                            + BITMAP_WIDTH as i32 * block_pos.0 as i32
                            + bitmap_x as i32;
                            let bitzoomed_y1: i32 = TILE_WIDTH as i32 * BITMAP_WIDTH as i32 * tile_pos.1 as i32
                            + BITMAP_WIDTH as i32 * block_pos.1 as i32
                            + bitmap_y as i32;

                            // Bottom-right coordinates (add one bit length to each side)
                            let bitzoomed_x2 = bitzoomed_x1 + 1;
                            let bitzoomed_y2 = bitzoomed_y1 + 1;

                            // Convert these to latitude/longitude
                            let (_lng1, lat1) = utils::tile_x_y_to_lng_lat(
                                bitzoomed_x1,
                                bitzoomed_y1,
                                (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                            );
                            let (_lng2, lat2) = utils::tile_x_y_to_lng_lat(
                                bitzoomed_x2,
                                bitzoomed_y2,
                                (BITMAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                            );

                            /* formula derived from spherical geometry of Earth */
                            /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference in latitudes in radians. */
                            let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();
                            /* use height as width for approximation. */
                            Some(height * height)
                        } else {
                            None
                        }
                    })
                })
            })
        })
        .sum();

    if total_area > 0.0 {
        Some(total_area)
    } else {
        None
    }
}

/*** use this method, better performance and accuracy ***/
pub fn get_area_by_journey_bitmap_interation_bit_estimate_block(
    journey_bitmap: &JourneyBitmap,
) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().filter_map(move |(block_pos, block)| {
                let mut bit_count = 0;
                for bitmap_x in 0..BITMAP_WIDTH {
                    for bitmap_y in 0..BITMAP_WIDTH {
                        if block.is_visited(bitmap_x as u8, bitmap_y as u8) {
                            bit_count += 1;
                        }
                    }
                }
                if bit_count > 0 {
                    // calculate center bit in block for bit_unit_area
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
                    /* width=R⋅Δλ⋅cos(ϕ), where Δλ = λ2-λ1 is the difference in longitudes in radians, ϕ is the latitude in radians*/
                    let width = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * lat1.to_radians().cos();
                    /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference in latitudes in radians. */
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

pub fn get_area_by_journey_bitmap_interation_block(journey_bitmap: &JourneyBitmap) -> Option<f64> {
    let total_area: f64 = journey_bitmap
        .tiles
        .iter()
        .flat_map(|(tile_pos, tile)| {
            tile.blocks.iter().filter_map(move |(block_pos, block)| {
                let mut bit_count = 0;
                for bitmap_x in 0..BITMAP_WIDTH {
                    for bitmap_y in 0..BITMAP_WIDTH {
                        if block.is_visited(bitmap_x as u8, bitmap_y as u8) {
                            bit_count += 1;
                        }
                    }
                }
                if bit_count > 0 {
                    // Calculate the top-left coordinates of this block
                    let blockzoomed_x1: i32 = TILE_WIDTH as i32 * tile_pos.0 as i32 + block_pos.0 as i32;
                    let blockzoomed_y1: i32 = TILE_WIDTH as i32 * tile_pos.1 as i32 + block_pos.1 as i32;
                    let (lng1, lat1) = utils::tile_x_y_to_lng_lat(
                        blockzoomed_x1,
                        blockzoomed_y1,
                        (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                    );
                    // Bottom-right coordinates (add one block length to each side)
                    let blockzoomed_x2 = blockzoomed_x1 + 1;
                    let blockzoomed_y2 = blockzoomed_y1 + 1;
                    let (lng2, lat2) = utils::tile_x_y_to_lng_lat(
                        blockzoomed_x2,
                        blockzoomed_y2,
                        (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                    );
                    let avg_lat = (lat1 + lat2) / 2.0;
                    /* formula derived from spherical geometry of Earth */
                    /* width=R⋅Δλ⋅cos(ϕ), where Δλ = λ2-λ1 is the difference in longitudes in radians, ϕ is the latitude in radians*/
                    let width = EARTH_RADIUS * (lng2 - lng1).abs().to_radians() * avg_lat.to_radians().cos();
                    /* height=R⋅Δφ, where Δφ = φ2-φ1 is the difference in latitudes in radians. */
                    let height = EARTH_RADIUS * (lat2 - lat1).abs().to_radians();
                    let block_area = width * height * bit_count as f64 / (BITMAP_WIDTH * BITMAP_WIDTH) as f64;
                    Some(block_area)
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
