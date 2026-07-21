use crate::journey_bitmap::{
    JourneyBitmap, MAP_WIDTH, MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET,
};
use crate::utils;

pub mod map_renderer;
pub use map_renderer::MapRenderer;

pub mod internal_server;

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapBoundsInternal {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Returns the smallest Web Mercator-aligned bounds containing every occupied
/// block. Longitude is circular: `east` may be greater than 180 degrees when
/// that is the narrow representation of a journey crossing the antimeridian.
pub fn get_bounds_from_journey_bitmap(journey_bitmap: &JourneyBitmap) -> Option<MapBoundsInternal> {
    let block_zoom = (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32;
    let world_width = (MAP_WIDTH * TILE_WIDTH) as usize;
    let mut occupied_x = vec![false; world_width];
    let mut min_y: Option<i32> = None;
    let mut max_y: Option<i32> = None;

    for tile_key in journey_bitmap.all_tile_keys() {
        journey_bitmap.peek_tile_without_updating_cache(tile_key, |tile| {
            let Some(tile) = tile else { return };
            for (block_key, _) in tile.iter() {
                let x = TILE_WIDTH as usize * tile_key.x as usize + block_key.x() as usize;
                let y = TILE_WIDTH as i32 * tile_key.y as i32 + block_key.y() as i32;
                if x >= world_width || y < 0 || y >= world_width as i32 {
                    continue;
                }
                occupied_x[x] = true;
                min_y = Some(min_y.map_or(y, |current| current.min(y)));
                max_y = Some(max_y.map_or(y, |current| current.max(y)));
            }
        });
    }

    let min_y = min_y?;
    let max_y = max_y.expect("max_y is set together with min_y");
    let occupied_columns: Vec<usize> = occupied_x
        .iter()
        .enumerate()
        .filter_map(|(x, occupied)| occupied.then_some(x))
        .collect();

    // Remove the largest empty gap on the circular x axis. The remaining arc
    // is the narrowest interval containing every occupied block column.
    let mut largest_gap = 0;
    let mut west_x = occupied_columns[0];
    let mut east_x = occupied_columns[0] + 1;
    for (index, &current) in occupied_columns.iter().enumerate() {
        let next = if index + 1 < occupied_columns.len() {
            occupied_columns[index + 1]
        } else {
            occupied_columns[0] + world_width
        };
        let gap = next - current - 1;
        if index == 0 || gap > largest_gap {
            largest_gap = gap;
            west_x = next % world_width;
            east_x = current + 1;
            if east_x <= west_x {
                east_x += world_width;
            }
        }
    }

    let (west, north) = utils::tile_x_y_to_lng_lat(west_x as i32, min_y, block_zoom);
    let (east, south) = utils::tile_x_y_to_lng_lat(east_x as i32, max_y + 1, block_zoom);

    Some(MapBoundsInternal {
        west,
        south,
        east,
        north,
    })
}

mod tile_shader2;
