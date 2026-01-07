use crate::journey_bitmap::{JourneyBitmap, MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET};
use crate::utils;

pub mod map_renderer;
pub use map_renderer::MapRenderer;

pub mod internal_server;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraOptionInternal {
    pub zoom: f64,
    pub lng: f64,
    pub lat: f64,
}

// TODO: redesign this interface at a better position
pub fn get_default_camera_option_from_journey_bitmap(
    journey_bitmap: &JourneyBitmap,
) -> Option<CameraOptionInternal> {
    // TODO: Currently we use the coordinate of the top left of a random block (first one in the hashtbl),
    // then just pick a hardcoded zoom level.
    // A better version could be finding a bounding box (need to be careful with the antimeridian).
    journey_bitmap
        .tiles
        .iter()
        .next()
        .and_then(|(tile_pos, tile)| {
            // we shouldn't have empty tile or block
            tile.iter().next().map(|(block_key, _)| {
                let blockzoomed_x: i32 =
                    TILE_WIDTH as i32 * tile_pos.0 as i32 + block_key.x() as i32;
                let blockzoomed_y: i32 =
                    TILE_WIDTH as i32 * tile_pos.1 as i32 + block_key.y() as i32;
                let (lng, lat) = utils::tile_x_y_to_lng_lat(
                    blockzoomed_x,
                    blockzoomed_y,
                    (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                );
                CameraOptionInternal {
                    zoom: 12.0,
                    lng,
                    lat,
                }
            })
        })
}

mod tile_shader2;
