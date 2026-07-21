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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey_bitmap::{Block, BlockKey, TileKey, BITMAP_SIZE};

    fn bitmap_with_blocks(blocks: &[(TileKey, BlockKey)]) -> JourneyBitmap {
        let mut bitmap = JourneyBitmap::new();
        for (tile_key, block_key) in blocks {
            let tile = bitmap.get_tile_mut_or_insert_empty(tile_key);
            let mut data = [0; BITMAP_SIZE];
            data[0] = 1;
            tile.set(block_key, Block::new_with_data(data));
        }
        bitmap
    }

    #[test]
    fn empty_bitmap_has_no_bounds() {
        assert_eq!(get_bounds_from_journey_bitmap(&JourneyBitmap::new()), None);
    }

    #[test]
    fn empty_tiles_do_not_hide_occupied_tiles() {
        let mut bitmap = bitmap_with_blocks(&[(TileKey::new(10, 20), BlockKey::from_x_y(5, 6))]);
        bitmap.get_tile_mut_or_insert_empty(&TileKey::new(30, 40));

        assert!(get_bounds_from_journey_bitmap(&bitmap).is_some());
    }

    #[test]
    fn bounds_include_complete_edge_blocks() {
        let bitmap = bitmap_with_blocks(&[
            (TileKey::new(10, 20), BlockKey::from_x_y(5, 6)),
            (TileKey::new(12, 23), BlockKey::from_x_y(7, 8)),
        ]);
        let bounds = get_bounds_from_journey_bitmap(&bitmap).unwrap();
        let zoom = (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32;
        let (expected_west, expected_north) = utils::tile_x_y_to_lng_lat(
            10 * TILE_WIDTH as i32 + 5,
            20 * TILE_WIDTH as i32 + 6,
            zoom,
        );
        let (expected_east, expected_south) = utils::tile_x_y_to_lng_lat(
            12 * TILE_WIDTH as i32 + 8,
            23 * TILE_WIDTH as i32 + 9,
            zoom,
        );

        assert_eq!(
            bounds,
            MapBoundsInternal {
                west: expected_west,
                south: expected_south,
                east: expected_east,
                north: expected_north,
            }
        );
    }

    #[test]
    fn antimeridian_crossing_uses_narrow_wrapped_bounds() {
        let bitmap = bitmap_with_blocks(&[
            (TileKey::new(511, 255), BlockKey::from_x_y(127, 10)),
            (TileKey::new(0, 255), BlockKey::from_x_y(0, 11)),
        ]);
        let bounds = get_bounds_from_journey_bitmap(&bitmap).unwrap();

        assert!(bounds.west > 179.0);
        assert!(bounds.east > 180.0);
        assert!(bounds.east - bounds.west < 0.02);
    }

    #[test]
    fn bounds_do_not_depend_on_tile_insertion_order() {
        let first = [
            (TileKey::new(300, 200), BlockKey::from_x_y(2, 3)),
            (TileKey::new(100, 220), BlockKey::from_x_y(4, 5)),
        ];
        let second = [first[1], first[0]];

        assert_eq!(
            get_bounds_from_journey_bitmap(&bitmap_with_blocks(&first)),
            get_bounds_from_journey_bitmap(&bitmap_with_blocks(&second))
        );
    }
}

mod tile_shader2;
