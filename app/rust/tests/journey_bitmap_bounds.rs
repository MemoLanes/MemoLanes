use memolanes_core::{
    journey_bitmap::{
        Block, BlockKey, JourneyBitmap, TileKey, BITMAP_SIZE, MAP_WIDTH_OFFSET, TILE_WIDTH,
        TILE_WIDTH_OFFSET,
    },
    utils::{self, get_bounds_from_journey_bitmap, MapBounds},
};

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
    assert_eq!(
        get_bounds_from_journey_bitmap(&mut JourneyBitmap::new()),
        None
    );
}

#[test]
fn empty_interior_tiles_do_not_affect_bounds() {
    let mut bitmap = bitmap_with_blocks(&[
        (TileKey::new(10, 20), BlockKey::from_x_y(5, 6)),
        (TileKey::new(30, 40), BlockKey::from_x_y(7, 8)),
    ]);
    bitmap.get_tile_mut_or_insert_empty(&TileKey::new(20, 30));

    assert!(get_bounds_from_journey_bitmap(&mut bitmap).is_some());
}

#[test]
fn bounds_include_complete_edge_blocks() {
    let mut bitmap = bitmap_with_blocks(&[
        (TileKey::new(10, 20), BlockKey::from_x_y(5, 6)),
        (TileKey::new(12, 23), BlockKey::from_x_y(7, 8)),
    ]);
    let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
    let zoom = (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32;
    let (expected_west, expected_north) =
        utils::tile_x_y_to_lng_lat(10 * TILE_WIDTH as i32 + 5, 20 * TILE_WIDTH as i32 + 6, zoom);
    let (expected_east, expected_south) =
        utils::tile_x_y_to_lng_lat(12 * TILE_WIDTH as i32 + 8, 23 * TILE_WIDTH as i32 + 9, zoom);

    assert_eq!(
        bounds,
        MapBounds {
            west: expected_west,
            south: expected_south,
            east: expected_east,
            north: expected_north,
        }
    );
}

#[test]
fn antimeridian_crossing_uses_narrow_wrapped_bounds() {
    let mut bitmap = bitmap_with_blocks(&[
        (TileKey::new(511, 255), BlockKey::from_x_y(127, 10)),
        (TileKey::new(0, 255), BlockKey::from_x_y(0, 11)),
    ]);
    let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();

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

    let mut first = bitmap_with_blocks(&first);
    let mut second = bitmap_with_blocks(&second);
    assert_eq!(
        get_bounds_from_journey_bitmap(&mut first),
        get_bounds_from_journey_bitmap(&mut second)
    );
}
