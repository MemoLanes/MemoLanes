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
        let mut editor = bitmap.edit();
        let tile = editor.get_tile_mut_or_insert_empty(tile_key);
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
        (TileKey::mercator(10, 20), BlockKey::from_x_y(5, 6)),
        (TileKey::mercator(30, 40), BlockKey::from_x_y(7, 8)),
    ]);
    bitmap
        .edit()
        .get_tile_mut_or_insert_empty(&TileKey::mercator(20, 30));

    assert!(get_bounds_from_journey_bitmap(&mut bitmap).is_some());
}

#[test]
fn bounds_include_complete_edge_blocks() {
    let mut bitmap = bitmap_with_blocks(&[
        (TileKey::mercator(10, 20), BlockKey::from_x_y(5, 6)),
        (TileKey::mercator(12, 23), BlockKey::from_x_y(7, 8)),
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
        (TileKey::mercator(511, 255), BlockKey::from_x_y(127, 10)),
        (TileKey::mercator(0, 255), BlockKey::from_x_y(0, 11)),
    ]);
    let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();

    assert!(bounds.west > 179.0);
    assert!(bounds.east > 180.0);
    assert!(bounds.east - bounds.west < 0.02);
}

#[test]
fn bounds_do_not_depend_on_tile_insertion_order() {
    let first = [
        (TileKey::mercator(300, 200), BlockKey::from_x_y(2, 3)),
        (TileKey::mercator(100, 220), BlockKey::from_x_y(4, 5)),
    ];
    let second = [first[1], first[0]];

    let mut first = bitmap_with_blocks(&first);
    let mut second = bitmap_with_blocks(&second);
    assert_eq!(
        get_bounds_from_journey_bitmap(&mut first),
        get_bounds_from_journey_bitmap(&mut second)
    );
}

#[test]
fn polar_only_bounds_follow_the_occupied_blocks() {
    for sign in [-1.0, 1.0] {
        let mut bitmap = JourneyBitmap::new();
        bitmap
            .edit()
            .add_line(10.0, sign * 89.0, 10.01, sign * 89.01);
        let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
        assert!(bounds.west <= 10.0 && bounds.east >= 10.01);
        assert!(bounds.east - bounds.west < 0.1);
        let (near, far) = if sign > 0.0 {
            (bounds.south, bounds.north)
        } else {
            (-bounds.north, -bounds.south)
        };
        assert!(near <= 89.0 && near > 88.99);
        assert!(far >= 89.01 && far < 89.02);
    }
}

#[test]
fn mixed_mercator_and_polar_bounds_keep_a_local_journey_local() {
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().add_line(10.0, 85.0, 10.0, 85.06);
    let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
    assert!(bounds.west <= 10.0 && bounds.east >= 10.0);
    assert!(bounds.east - bounds.west < 0.1);
    assert!(bounds.south <= 85.0 && bounds.south > 84.99);
    assert!(bounds.north >= 85.06 && bounds.north < 85.07);
}

#[test]
fn polar_dateline_crossings_use_narrow_wrapped_bounds() {
    for sign in [-1.0, 1.0] {
        let mut bitmap = JourneyBitmap::new();
        bitmap
            .edit()
            .add_line(179.9, sign * 89.0, -179.9, sign * 89.0);
        let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
        assert!(bounds.west <= 179.9 && bounds.west > 179.8);
        assert!(bounds.east >= 180.1 && bounds.east < 180.2);
    }
}

#[test]
fn polar_bounds_span_all_longitudes_only_when_coverage_surrounds_the_pole() {
    for latitude in [-90.0, 90.0] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(0.0, latitude, 0.0, latitude);
        let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
        assert_eq!((bounds.west, bounds.east), (-180.0, 180.0));
        if latitude > 0.0 {
            assert_eq!(bounds.north, 90.0);
            assert!(bounds.south > 89.99);
        } else {
            assert_eq!(bounds.south, -90.0);
            assert!(bounds.north < -89.99);
        }
    }
}

#[test]
fn mixed_bounds_preserve_wide_intervals_and_do_not_depend_on_tile_loading() {
    use memolanes_core::journey_data::{deserialize_journey_bitmap, serialize_journey_bitmap};
    let mut bitmap = JourneyBitmap::new();
    for longitude in [-170.0, -50.0, 70.0] {
        bitmap.edit().add_line(longitude, 0.0, longitude, 0.0);
    }
    bitmap.edit().add_line(10.0, 89.0, 10.01, 89.01);
    bitmap.edit().add_line(10.0, -89.0, 10.01, -89.01);
    let bounds = get_bounds_from_journey_bitmap(&mut bitmap).unwrap();
    assert!(bounds.east - bounds.west >= 240.0);
    assert!(bounds.south > -89.02 && bounds.north < 89.02);
    let mut bytes = Vec::new();
    serialize_journey_bitmap(&mut bitmap, &mut bytes).unwrap();
    let mut lazy = deserialize_journey_bitmap(&bytes[..], false).unwrap();
    assert_eq!(get_bounds_from_journey_bitmap(&mut lazy), Some(bounds));
}
