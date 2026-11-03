pub mod test_utils;
use crate::test_utils::{
    draw_line1, draw_line2, draw_line3, draw_line4, END_LAT, END_LNG, START_LAT, START_LNG,
};
use memolanes_core::{
    gps_processor::SegmentGapRule,
    import_data,
    journey_bitmap::area,
    journey_bitmap::{
        BitmapPlane, Block, BlockKey, JourneyBitmap, Tile, TileKey, BITMAP_WIDTH, MAP_WIDTH,
        POLAR_MAP_WIDTH, TILE_WIDTH,
    },
    journey_data::JourneyData,
    journey_header::JourneyType,
    renderer::MapRenderer,
};

#[test]
fn add_line_cross_antimeridian() {
    let mut journey_bitmap = JourneyBitmap::new();

    // Melbourne to Hawaii
    let (start_lng, start_lat, end_lng, end_lat) =
        (144.847737, 37.6721702, -160.3644029, 21.3186185);
    journey_bitmap
        .edit()
        .add_line(start_lng, start_lat, end_lng, end_lat);

    // Hawaii to Guan
    let (start_lng, start_lat, end_lng, end_lat) =
        (-160.3644029, 21.3186185, 121.4708788, 9.4963078);
    journey_bitmap
        .edit()
        .add_line(start_lng, start_lat, end_lng, end_lat);

    let (start_lng, start_lat, end_lng, end_lat) =
        (175.4708788, 5.4963078, -175.3644029, -28.3186185);
    journey_bitmap
        .edit()
        .add_line(start_lng, start_lat, end_lng, end_lat);

    let (start_lng, start_lat, end_lng, end_lat) =
        (-175.3644029, -28.3186185, 175.4708788, -49.4963078);
    journey_bitmap
        .edit()
        .add_line(start_lng, start_lat, end_lng, end_lat);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&mut map_renderer, 0, -170.0, 80.0, 170.0, -80.0);
    test_utils::verify_image(
        "journey_bitmap_add_line_cross_antimeridian",
        &render_result.data,
    );
}

#[test]
fn add_line_stores_tracks_in_the_correct_projection_plane() {
    for (lng, lat, plane) in [
        (0.0, 88.0, BitmapPlane::NorthPolar),
        (120.0, 88.0, BitmapPlane::NorthPolar),
        (0.0, -87.0, BitmapPlane::SouthPolar),
    ] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(lng, lat, lng + 0.01, lat + 0.01);
        assert!(
            bitmap
                .all_plane_tile_keys(BitmapPlane::Mercator)
                .next()
                .is_none(),
            "polar-only line must not leak into Mercator"
        );
        assert!(bitmap.all_plane_tile_keys(plane).next().is_some());
        assert!(bitmap.all_plane_tile_keys(plane).all(|key| matches!(
            *key,
            TileKey::NorthPolar { x, y } | TileKey::SouthPolar { x, y }
                if x < POLAR_MAP_WIDTH as u16 && y < POLAR_MAP_WIDTH as u16
        )));
    }

    let mut clipped = JourneyBitmap::new();
    clipped.edit().add_line(0.0, 84.0, 0.0, 88.0);
    assert!(!clipped.is_empty());
    assert!(clipped
        .all_plane_tile_keys(BitmapPlane::Mercator)
        .any(|key| matches!(key, TileKey::Mercator { y: 0, .. })));
    assert!(clipped
        .all_plane_tile_keys(BitmapPlane::NorthPolar)
        .next()
        .is_some());

    let mut wrapped = JourneyBitmap::new();
    wrapped.edit().add_line(-181.0, 0.0, -180.99, 0.0);
    let mut canonical = JourneyBitmap::new();
    canonical.edit().add_line(179.0, 0.0, 179.01, 0.0);
    assert_eq!(wrapped, canonical);
    assert!([&clipped, &wrapped]
        .into_iter()
        .flat_map(|bitmap| bitmap.all_tile_keys())
        .all(|key| match *key {
            TileKey::Mercator { x, y } => x < MAP_WIDTH as u16 && y < MAP_WIDTH as u16,
            TileKey::NorthPolar { x, y } | TileKey::SouthPolar { x, y } => {
                x < POLAR_MAP_WIDTH as u16 && y < POLAR_MAP_WIDTH as u16
            }
        }));
}

#[test]
fn polar_point_brush_crosses_tile_corners_and_invalidates_neighbor_areas() {
    let tile_side = TILE_WIDTH * BITMAP_WIDTH;
    let center = POLAR_MAP_WIDTH * tile_side / 2;
    for (latitude, plane) in [
        (90.0, BitmapPlane::NorthPolar),
        (-90.0, BitmapPlane::SouthPolar),
    ] {
        let mut renderer = MapRenderer::new(JourneyBitmap::new());
        assert_eq!(renderer.get_current_area(), 0);
        renderer.update(|editor| editor.add_line(0.0, latitude, 0.0, latitude));

        let bitmap = renderer.peek_latest_bitmap();
        assert_eq!(bitmap.tile_count(), 4);
        // A nine-pixel square centred on either pole straddles four tiles.
        for x in center - 4..=center + 4 {
            for y in center - 4..=center + 4 {
                let tile_key = TileKey::new(plane, (x / tile_side) as u16, (y / tile_side) as u16);
                let block_key = BlockKey::from_x_y(
                    ((x / BITMAP_WIDTH) % TILE_WIDTH) as u8,
                    ((y / BITMAP_WIDTH) % TILE_WIDTH) as u8,
                );
                assert!(bitmap.peek_tile_without_updating_cache(&tile_key, |tile| {
                    tile.and_then(|tile| tile.get(&block_key))
                        .is_some_and(|block| {
                            block.is_visited((x % BITMAP_WIDTH) as u8, (y % BITMAP_WIDTH) as u8)
                        })
                }));
            }
        }
        // Each polar pixel is 11010 cm², so the complete 9×9 point rounds to 89 m².
        assert_eq!(renderer.get_current_area(), 89);

        // This point projects one pixel east of the pole. Its new column spans
        // two already-cached tiles, both of which must invalidate their areas.
        let nearby_latitude = latitude.signum() * 89.99999;
        renderer.update(|editor| editor.add_line(90.0, nearby_latitude, 90.0, nearby_latitude));
        assert_eq!(renderer.get_current_area(), 99); // 10×9 pixels
        let version = renderer.get_current_version();
        renderer.update(|editor| editor.add_line(0.0, latitude, 0.0, latitude));
        assert_eq!(renderer.get_current_version(), version);
        assert_eq!(renderer.get_current_area(), 99);
    }
}

fn covers_geographic_point(bitmap: &JourneyBitmap, longitude: f64, latitude: f64) -> bool {
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    let longitude = (longitude + 180.0).rem_euclid(360.0) - 180.0;
    let (plane, x, y) = if latitude.abs() <= MERCATOR_MAX_LATITUDE {
        let (x, y) = memolanes_core::utils::lng_lat_to_tile_x_y(longitude, latitude, 22);
        // Projection roundoff at the exact seam can put y just outside the grid.
        let last_row = MAP_WIDTH * TILE_WIDTH * BITMAP_WIDTH - 1;
        (
            BitmapPlane::Mercator,
            i64::from(x),
            i64::from(y).clamp(0, last_row),
        )
    } else {
        let radius = POLAR_MAP_WIDTH * TILE_WIDTH * BITMAP_WIDTH / 2;
        let seam_radius = ((90.0 - MERCATOR_MAX_LATITUDE).to_radians() / 2.0).sin();
        let r = ((90.0 - latitude.abs()).to_radians() / 2.0).sin() / seam_radius * radius as f64;
        let angle = longitude.to_radians();
        // Query the containing cell; only brush centres are rounded when drawing.
        let x = (radius as f64 + r * angle.sin()).floor() as i64;
        let y = (radius as f64 - latitude.signum() * r * angle.cos()).floor() as i64;
        let plane = if latitude > 0.0 {
            BitmapPlane::NorthPolar
        } else {
            BitmapPlane::SouthPolar
        };
        (plane, x, y)
    };
    let tile_side = TILE_WIDTH * BITMAP_WIDTH;
    let tile_key = TileKey::new(plane, (x / tile_side) as u16, (y / tile_side) as u16);
    let block_key = BlockKey::from_x_y(
        ((x / BITMAP_WIDTH) % TILE_WIDTH) as u8,
        ((y / BITMAP_WIDTH) % TILE_WIDTH) as u8,
    );
    bitmap.peek_tile_without_updating_cache(&tile_key, |tile| {
        tile.and_then(|tile| tile.get(&block_key))
            .is_some_and(|block| {
                block.is_visited((x % BITMAP_WIDTH) as u8, (y % BITMAP_WIDTH) as u8)
            })
    })
}

#[test]
fn seam_point_brush_covers_both_charts_including_the_dateline() {
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    for sign in [-1.0, 1.0] {
        for longitude in [0.0, 45.123, 179.99999] {
            for offset in [-0.000001, 0.0, 0.000001] {
                let latitude = sign * (MERCATOR_MAX_LATITUDE + offset);
                let mut bitmap = JourneyBitmap::new();
                bitmap
                    .edit()
                    .add_line(longitude, latitude, longitude, latitude);
                // Include the exact Mercator edge and a cap point only 8 cm
                // inside the disk, where rounding can select an outside cell.
                // The outer samples are about 2.2 m to either side of the seam.
                for delta in [-0.00002, 0.0, 0.0000007, 0.00002] {
                    assert!(
                        covers_geographic_point(
                            &bitmap,
                            longitude + 0.00002,
                            sign * (MERCATOR_MAX_LATITUDE + delta),
                        ),
                        "longitude={longitude}, sign={sign}, offset={offset}, delta={delta}"
                    );
                }
                // Extending the projection must not clamp the centre to the seam
                // or otherwise create coverage outside the original footprint.
                for delta in [-0.0002, 0.0002] {
                    assert!(!covers_geographic_point(
                        &bitmap,
                        longitude,
                        sign * (MERCATOR_MAX_LATITUDE + delta),
                    ));
                }
            }
        }
    }
}

#[test]
fn seam_line_brush_crosses_charts_even_when_its_centreline_does_not() {
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    for sign in [-1.0, 1.0] {
        for offset in [-0.000001, 0.000001] {
            let latitude = sign * (MERCATOR_MAX_LATITUDE + offset);
            let mut bitmap = JourneyBitmap::new();
            bitmap.edit().add_line(45.1225, latitude, 45.1235, latitude);
            for longitude in [45.1226, 45.123, 45.1234] {
                for delta in [-0.00002, 0.00002] {
                    assert!(covers_geographic_point(
                        &bitmap,
                        longitude,
                        sign * (MERCATOR_MAX_LATITUDE + delta),
                    ));
                }
            }
            let mut reversed = JourneyBitmap::new();
            reversed
                .edit()
                .add_line(45.1235, latitude, 45.1225, latitude);
            assert_eq!(bitmap, reversed);
        }
    }
}

#[test]
fn seam_overlap_does_not_store_empty_coverage_in_the_adjacent_chart() {
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    for sign in [-1.0, 1.0] {
        for offset in [-0.00007, 0.00007] {
            // The centre is inside the overlap band but about 7.8 m from the
            // seam, beyond the reach of either brush at this longitude.
            let latitude = sign * (MERCATOR_MAX_LATITUDE + offset);
            let mut bitmap = JourneyBitmap::new();
            bitmap.edit().add_line(0.0, latitude, 0.0, latitude);
            assert!(!bitmap.is_empty());
            assert_eq!(bitmap.has_polar_data(), offset > 0.0);
            assert_eq!(bitmap.has_plane_data(BitmapPlane::Mercator), offset < 0.0);
        }
    }
}

#[test]
fn add_line_crossing_polar_seam_is_direction_independent() {
    for (outside_lat, inside_lat) in [(88.0, 84.0), (-88.0, -84.0)] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(0.0, outside_lat, 120.0, inside_lat);

        assert!(!bitmap.is_empty());
        assert!(bitmap.has_plane_data(BitmapPlane::Mercator));
        let polar_plane = if outside_lat > 0.0 {
            BitmapPlane::NorthPolar
        } else {
            BitmapPlane::SouthPolar
        };
        assert!(bitmap.all_plane_tile_keys(polar_plane).next().is_some());

        let mut reversed = JourneyBitmap::new();
        reversed
            .edit()
            .add_line(120.0, inside_lat, 0.0, outside_lat);
        assert_eq!(bitmap, reversed);
    }
}

#[test]
fn great_circle_can_enter_a_polar_cap_when_both_endpoints_are_below_the_seam() {
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().add_line(0.0, 84.0, 120.0, 84.0);

    assert!(bitmap.has_plane_data(BitmapPlane::Mercator));
    assert!(bitmap
        .all_plane_tile_keys(BitmapPlane::NorthPolar)
        .next()
        .is_some());
}

#[test]
fn add_line_rejects_latitudes_outside_the_geographic_domain() {
    for (start_lat, end_lat) in [(91.0, 91.0), (-91.0, -91.0), (0.0, 91.0), (-91.0, 0.0)] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(0.0, start_lat, 0.01, end_lat);
        assert!(
            bitmap.is_empty(),
            "line with latitudes ({start_lat}, {end_lat}) should be ignored"
        );
    }
}

#[test]
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap
        .edit()
        .add_line(START_LNG, START_LAT, END_LNG, START_LAT);
    journey_bitmap
        .edit()
        .add_line(END_LNG, END_LAT, START_LNG, END_LAT);
    journey_bitmap
        .edit()
        .add_line(START_LNG, START_LAT, START_LNG, END_LAT);
    journey_bitmap
        .edit()
        .add_line(END_LNG, END_LAT, END_LNG, START_LAT);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = test_utils::render_map_overlay(
        &mut map_renderer,
        12,
        START_LNG,
        START_LAT,
        END_LNG,
        END_LAT,
    );
    test_utils::verify_image("journey_bitmap_basic", &render_result.data);
}

#[test]
fn merge_with_render() {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);

    let mut other_journey_bitmap = JourneyBitmap::new();
    draw_line2(&mut other_journey_bitmap);

    journey_bitmap.edit().merge(other_journey_bitmap);

    let mut sample_journaey_bitmap = JourneyBitmap::new();
    draw_line1(&mut sample_journaey_bitmap);
    draw_line2(&mut sample_journaey_bitmap);
    assert_eq!(journey_bitmap, sample_journaey_bitmap);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = test_utils::render_map_overlay(
        &mut map_renderer,
        12,
        START_LNG,
        START_LAT,
        END_LNG,
        END_LAT,
    );
    test_utils::verify_image("journey_bitmap_merge_with_render", &render_result.data);
}

#[test]
fn intersection_and_difference() {
    // line3 - (line3 - line4) = line3 & line4

    let mut rhs = JourneyBitmap::new();
    draw_line3(&mut rhs);
    {
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        rhs.edit().intersection(&line4);
    }

    let mut lhs = JourneyBitmap::new();
    draw_line3(&mut lhs);
    {
        let mut line3 = JourneyBitmap::new();
        draw_line3(&mut line3);
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        line3.edit().difference(&line4);
        lhs.edit().difference(&line3);
    }

    assert_ne!(lhs, JourneyBitmap::new());
    assert_eq!(lhs, rhs);
}

#[test]
fn intersection_and_difference_produce_empty() {
    {
        // line1 - line1
        let mut journey_bitmap = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap);
        let mut line1 = JourneyBitmap::new();
        draw_line1(&mut line1);
        journey_bitmap.edit().difference(&line1);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
    {
        // line1 & line2
        let mut journey_bitmap = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap);
        let mut line2 = JourneyBitmap::new();
        draw_line2(&mut line2);
        journey_bitmap.edit().intersection(&line2);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
}

#[test]
fn serialization() {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    let mut journey_data = JourneyData::Bitmap(journey_bitmap);

    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();
    assert_eq!(&buf[..2], b"B0");

    println!("size: {}", buf.len());

    let journey_data_roundtrip =
        JourneyData::deserialize(buf.as_slice(), JourneyType::Bitmap, true).unwrap();
    assert_eq!(journey_data, journey_data_roundtrip);
}

#[test]
fn area_cache_distinguishes_planes_while_rendering_remains_mercator_only() {
    let make_tile = |count: u8| {
        let mut block = Block::new();
        for x in 0..count {
            block.set_point(x, 0, true);
        }
        let mut tile = Tile::new();
        tile.set(&BlockKey::from_x_y(0, 0), block);
        tile
    };
    let mut renderer = MapRenderer::new(JourneyBitmap::new());
    let render = |renderer: &mut MapRenderer| {
        renderer
            .get_tile_range_response(64, 64, 9, 1, 1, 10)
            .unwrap()
    };
    let mut mercator_render = None;
    let mut expected_cm2 = 0;
    for (i, plane) in BitmapPlane::ALL.into_iter().enumerate() {
        let key = TileKey::new(plane, 64, 64);
        let mut single_plane = JourneyBitmap::new();
        single_plane
            .edit()
            .insert_tile(&key, make_tile((i + 1) as u8));
        expected_cm2 += area::journey_bitmap_area_cm2(&single_plane, None);
        renderer.update(|editor| editor.merge(single_plane));
        assert_eq!(
            renderer.get_current_area(),
            area::cm2_to_m2_rounded(expected_cm2)
        );
        let rendered = render(&mut renderer);
        match plane {
            BitmapPlane::Mercator => mercator_render = Some(rendered),
            BitmapPlane::NorthPolar | BitmapPlane::SouthPolar => {
                assert_eq!(Some(rendered), mercator_render);
            }
        }
    }
    // Change one key after all three areas have been cached, then remove it.
    let north = TileKey::NorthPolar { x: 64, y: 64 };
    renderer.update(|editor| editor.insert_tile(&north, make_tile(20)));
    expected_cm2 += 18 * 11010;
    assert_eq!(
        renderer.get_current_area(),
        area::cm2_to_m2_rounded(expected_cm2)
    );
    assert_eq!(Some(render(&mut renderer)), mercator_render);
    let mut removal = JourneyBitmap::new();
    removal.edit().insert_tile(&north, make_tile(20));
    renderer.update(|editor| editor.difference(&removal));
    expected_cm2 -= 20 * 11010;
    assert_eq!(
        renderer.get_current_area(),
        area::cm2_to_m2_rounded(expected_cm2)
    );
    assert_eq!(Some(render(&mut renderer)), mercator_render);
}

#[test]
fn tagged_b0_keeps_the_legacy_record_layout_and_mercator_bytes() {
    use integer_encoding::VarInt;

    // Wire coordinates are explicit format fixtures, independent of the codec.
    let records = [
        (TileKey::Mercator { x: 0, y: 0 }, 0x0000_u16, 0x0000_u16),
        (TileKey::Mercator { x: 511, y: 511 }, 0x01ff, 0x01ff),
        (TileKey::NorthPolar { x: 0, y: 0 }, 0x0200, 0x0000),
        (TileKey::NorthPolar { x: 127, y: 127 }, 0x027f, 0x007f),
        (TileKey::SouthPolar { x: 0, y: 0 }, 0x0400, 0x0000),
        (TileKey::SouthPolar { x: 127, y: 127 }, 0x047f, 0x007f),
    ];
    let mut block = Block::new();
    block.set_point(10, 20, true);
    let mut tile = Tile::new();
    tile.set(&BlockKey::from_x_y(0, 0), block);
    let payload = tile.serialize();

    // The first fixture is an unextended legacy B0; the second adds both caps.
    for records in [&records[..2], &records[..]] {
        let mut fixture = b"B0".to_vec();
        fixture.extend((records.len() as u64).encode_var_vec());
        for &(_, x, y) in records {
            fixture.extend(x.to_be_bytes());
            fixture.extend(y.to_be_bytes());
            fixture.extend((payload.len() as u64).encode_var_vec());
            fixture.extend(&payload);
        }

        let mut expected = JourneyBitmap::new();
        for &(key, _, _) in records.iter().rev() {
            expected.edit().insert_tile(&key, tile.clone());
        }
        let mut encoded = Vec::new();
        memolanes_core::journey_data::serialize_journey_bitmap(&mut expected, &mut encoded)
            .unwrap();
        assert_eq!(encoded, fixture);
        for full_validation in [false, true] {
            let decoded = memolanes_core::journey_data::deserialize_journey_bitmap(
                &fixture[..],
                full_validation,
            )
            .unwrap();
            assert_eq!(decoded, expected);
        }
    }
}

#[test]
fn bitmap_deserialization_handles_large_inputs_and_truncated_record_counts() {
    use integer_encoding::VarInt;
    use memolanes_core::journey_data::{deserialize_journey_bitmap, serialize_journey_bitmap};

    let mut block = Block::new();
    block.set_point(10, 20, true);
    let mut tile = Tile::new();
    tile.set(&BlockKey::from_x_y(0, 0), block);
    let payload = tile.serialize();
    let mut bytes = b"B0".to_vec();
    bytes.extend(4096_u64.encode_var_vec());
    for x in 0..32_u16 {
        for y in 0..128_u16 {
            bytes.extend(x.to_be_bytes());
            bytes.extend(y.to_be_bytes());
            bytes.extend((payload.len() as u64).encode_var_vec());
            bytes.extend(&payload);
        }
    }
    let mut bitmap = deserialize_journey_bitmap(&bytes[..], false).unwrap();
    assert_eq!(bitmap.tile_count(), 4096);
    let mut roundtrip = Vec::new();
    serialize_journey_bitmap(&mut bitmap, &mut roundtrip).unwrap();
    assert_eq!(roundtrip, bytes);

    // An oversized count with no records must fail reading, without attempting
    // to allocate capacity for the entire claimed input.
    let mut truncated = b"B0".to_vec();
    truncated.extend((usize::MAX as u64).encode_var_vec());
    assert!(deserialize_journey_bitmap(&truncated[..], false).is_err());
}

#[test]
fn tagged_b0_does_not_mask_reserved_x_bits_into_valid_tiles() {
    for encoded_x in [0x0600_u16, 0x0800, 0x8200] {
        let mut bytes = b"B0\x01".to_vec();
        bytes.extend(encoded_x.to_be_bytes());
        bytes.extend([0, 0, 0]); // y = 0, empty payload
        let error = memolanes_core::journey_data::deserialize_journey_bitmap(&bytes[..], false)
            .unwrap_err();
        assert!(format!("{error:#}").contains("Invalid bitmap tile namespace"));
    }
}

#[test]
fn polar_serialization_uses_tagged_b0_and_roundtrips() {
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().add_line(114.0, 22.0, 114.01, 22.01);
    bitmap.edit().add_line(45.0, 87.0, 46.0, 87.5);
    bitmap.edit().add_line(-30.0, -88.0, -29.0, -88.5);
    let expected = bitmap.clone();
    let mut data = JourneyData::Bitmap(bitmap);
    let mut bytes = Vec::new();
    data.serialize(&mut bytes).unwrap();
    assert_eq!(&bytes[..2], b"B0");
    let decoded = JourneyData::deserialize(&bytes[..], JourneyType::Bitmap, true).unwrap();
    assert_eq!(decoded, JourneyData::Bitmap(expected));
}

#[test]
fn polar_gpx_import_matches_bitmap_area_snapshot() {
    let (raw_data, _) = import_data::gpx::load_gpx("./tests/data/polar_complex.gpx").unwrap();

    let vector = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data, None,
    )
    .unwrap();
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(&vector);
    for plane in BitmapPlane::ALL {
        assert!(bitmap.has_plane_data(plane), "{plane:?} must be populated");
    }

    // Regression snapshot of the full import/rasterization path, in cm².
    // Update only after reviewing intentional changes to the covered geometry.
    // Complete cross-partition brushes retain formerly clipped edge pixels.
    assert_eq!(area::journey_bitmap_area_cm2(&bitmap, None), 1_218_281_635,);
}

#[test]
fn polar_gpx_seams_preserve_the_track_interior_and_render_continuously() {
    use image::{Rgb, RgbImage};
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    use std::io::Cursor;

    let (raw, _) = import_data::gpx::load_gpx("./tests/data/polar_complex.gpx").unwrap();
    let vector =
        import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(&raw, None)
            .unwrap();
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(&vector);
    // Every seam-weave segment is under 200 m. Local interpolation differs
    // from its spherical arc by far less than a source cell. Check both sides
    // of its interior, so a clipped half-brush cannot pass as a connected line.
    const R: f64 = 6_371_000.0;
    for segment in &vector.track_segments {
        if segment.track_points.iter().any(|p| p.latitude.abs() > 86.0) {
            continue;
        }
        for pair in segment.track_points.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            let longitude_delta = (b.longitude - a.longitude + 180.0).rem_euclid(360.0) - 180.0;
            let scale = ((a.latitude + b.latitude) * 0.5).to_radians().cos();
            let dx = longitude_delta.to_radians() * R * scale;
            let dy = (b.latitude - a.latitude).to_radians() * R;
            let length = dx.hypot(dy);
            let steps = length.ceil() as usize;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                for offset in [-2.5, 0.0, 2.5] {
                    let lng = a.longitude + longitude_delta * t
                        - (offset * dy / length / R / scale).to_degrees();
                    let lat = a.latitude
                        + (b.latitude - a.latitude) * t
                        + (offset * dx / length / R).to_degrees();
                    assert!(
                        covers_geographic_point(&bitmap, lng, lat),
                        "Missing track interior at {lng}, {lat}, offset={offset}"
                    );
                }
            }
        }
    }

    let mut image = RgbImage::new(800, 620);
    for (panel, sign) in [1.0, -1.0].into_iter().enumerate() {
        for y in 0..620 {
            for x in 0..400 {
                let lng = 180.0
                    + (((x as f64 - 200.0) * 0.8) / R / MERCATOR_MAX_LATITUDE.to_radians().cos())
                        .to_degrees();
                let lat =
                    sign * (MERCATOR_MAX_LATITUDE + (((310.0 - y as f64) * 0.8) / R).to_degrees());
                let color = if covers_geographic_point(&bitmap, lng, lat) {
                    [0, 100, 160]
                } else if y == 310 {
                    [210, 210, 210]
                } else {
                    [255, 255, 255]
                };
                image.put_pixel(panel as u32 * 400 + x, y, Rgb(color));
            }
        }
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    test_utils::verify_image("journey_bitmap_polar_seams", &png);
}

fn vector_to_bitmap(name: &str, zoom: i32, filename_override: Option<&str>) {
    let filename = match filename_override {
        None => format!("./tests/data/raw_gps_{name}.gpx"),
        Some(filename) => format!("./tests/data/{filename}"),
    };
    let (loaded_data, _preprocessor) = import_data::gpx::load_gpx(&filename).unwrap();
    let journey_vector =
        import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
            &loaded_data,
            Some(SegmentGapRule::Default),
        )
        .unwrap();
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.edit().merge_vector(&journey_vector);

    // compute the bounding box
    let (mut left, mut right, mut top, mut bottom): (f64, f64, f64, f64) = (180., -180., -90., 90.);
    for segment in &journey_vector.track_segments {
        for point in &segment.track_points {
            left = left.min(point.longitude);
            right = right.max(point.longitude);
            top = top.max(point.latitude);
            bottom = bottom.min(point.latitude);
        }
    }

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&mut map_renderer, zoom, left, top, right, bottom);
    test_utils::verify_image(
        &format!("journey_bitmap_vector_to_bitmap_{name}"),
        &render_result.data,
    );
}

// `raw_gps_shanghai.gpx` is already covered by the end to end test.

#[test]
fn vector_to_bitmap_shenzhen_stationary() {
    vector_to_bitmap("shenzhen_stationary", 16, None);
}

#[test]
fn vector_to_bitmap_laojunshan() {
    vector_to_bitmap("laojunshan", 16, None);
}

#[test]
fn vector_to_bitmap_heihe() {
    vector_to_bitmap("heihe", 13, None);
}

#[test]
fn vector_to_bitmap_nelson_to_wharariki_beach() {
    // `nelson_to_wharariki_beach.gps` is not a raw gps because it does not contian timestamps,
    // but it is good enough for this test.
    vector_to_bitmap(
        "nelson_to_wharariki_beach",
        9,
        Some("nelson_to_wharariki_beach.gpx"),
    );
}

#[test]
fn draw_single_point() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.edit().add_line(120.0, 30.0, 120.0, 30.0);

    assert_eq!(
        area::journey_bitmap_area_m2_rounded(&journey_bitmap, None),
        68
    );
}

#[test]
fn draw_line_in_different_latitude() {
    // base in lat 0
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.edit().add_line(120.0, 0.0, 120.0, 1.0);
    journey_bitmap.edit().add_line(120.0, 0.0, 121.0, 0.0);
    journey_bitmap.edit().add_line(120.0, 0.0, 121.0, 1.0);
    assert_eq!(
        area::journey_bitmap_area_m2_rounded(&journey_bitmap, None),
        3183812
    );

    // width is 2 , lat 60
    let mut journey_bitmap2 = JourneyBitmap::new();
    journey_bitmap2.edit().add_line(120.0, 60.0, 120.0, 61.0);
    journey_bitmap2.edit().add_line(120.0, 60.0, 122.0, 60.0);
    journey_bitmap2.edit().add_line(120.0, 60.0, 122.0, 61.0);
    let area2 = area::journey_bitmap_area_m2_rounded(&journey_bitmap2, None);
    assert_eq!(area2 / 100000, 31);

    // width is 3 , lat 70.5
    let mut journey_bitmap3 = JourneyBitmap::new();
    journey_bitmap3.edit().add_line(120.0, 70.5, 120.0, 71.5);
    journey_bitmap3.edit().add_line(120.0, 70.5, 123.0, 70.5);
    journey_bitmap3.edit().add_line(120.0, 70.5, 123.0, 71.5);
    let area3 = area::journey_bitmap_area_m2_rounded(&journey_bitmap3, None);
    assert_eq!(area3 / 100000, 31);

    // width is 3 , lat -70.5
    let mut journey_bitmap4 = JourneyBitmap::new();
    journey_bitmap4.edit().add_line(120.0, -70.5, 120.0, -71.5);
    journey_bitmap4.edit().add_line(120.0, -70.5, 123.0, -70.5);
    journey_bitmap4.edit().add_line(120.0, -70.5, 123.0, -71.5);
    let area3 = area::journey_bitmap_area_m2_rounded(&journey_bitmap4, None);
    assert_eq!(area3 / 100000, 31);
}

#[test]
fn draw_line_with_width2() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.edit().add_line(120.0, 60.0, 120.0, 60.005);
    journey_bitmap.edit().add_line(120.0, 60.005, 120.01, 60.01);
    journey_bitmap.edit().add_line(120.01, 60.01, 120.0, 60.0);
    journey_bitmap.edit().add_line(120.0, 60.0, 120.005, 60.0);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&mut map_renderer, 13, 120.0, 60.01, 120.01, 60.0);
    test_utils::verify_image("draw_line_with_width2", &render_result.data);
}

#[test]
fn draw_line_with_width3() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.edit().add_line(120.0, 70.0, 120.0, 70.005);
    journey_bitmap.edit().add_line(120.0, 70.005, 120.01, 70.01);
    journey_bitmap.edit().add_line(120.01, 70.01, 120.0, 70.0);
    journey_bitmap.edit().add_line(120.0, 70.0, 120.005, 70.0);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&mut map_renderer, 13, 120.0, 70.01, 120.01, 70.0);
    test_utils::verify_image("draw_line_with_width3", &render_result.data);
}

#[test]
fn unchecked_deserialization_discards_out_of_bounds_tiles() {
    for plane in BitmapPlane::ALL {
        let width = plane.grid_width() as u16;
        let valid_key = TileKey::new(plane, width - 1, width - 1);
        let invalid_x = TileKey::new(plane, width, 0);
        let invalid_y = TileKey::new(plane, 0, width);

        let bitmap = JourneyBitmap::of_tile_bytes_without_validation(vec![
            (invalid_x, vec![0xff]),
            (valid_key, Tile::new().serialize()),
            (invalid_y, vec![0xff]),
        ])
        .unwrap();

        assert_eq!(bitmap.tile_count(), 1);
        assert!(bitmap.contains_tile(&valid_key));
        assert!(!bitmap.contains_tile(&invalid_x));
        assert!(!bitmap.contains_tile(&invalid_y));
    }
}

#[test]
fn insert_tile_discards_out_of_bounds_tile() {
    let invalid_key = TileKey::mercator(u16::MAX, u16::MAX);
    let mut bitmap = JourneyBitmap::new();

    bitmap.edit().insert_tile(&invalid_key, Tile::new());

    assert!(bitmap.is_empty());
}

#[test]
#[should_panic(expected = "out-of-bounds bitmap tile")]
fn get_tile_mut_or_insert_empty_panics_for_out_of_bounds_tile() {
    JourneyBitmap::new()
        .edit()
        .get_tile_mut_or_insert_empty(&TileKey::mercator(0, MAP_WIDTH as u16));
}

#[test]
fn validate_prunes_empty_blocks_and_tiles() {
    let keep_key = TileKey::mercator(1, 2);
    let drop_key = TileKey::mercator(3, 4);
    let empty_block_key = BlockKey::from_x_y(0, 0);
    let non_empty_block_key = BlockKey::from_x_y(0, 1);

    let mut keep_tile = Tile::new();
    keep_tile.set(&empty_block_key, Block::new());
    let mut non_empty_block = Block::new();
    non_empty_block.set_point(10, 10, true);
    keep_tile.set(&non_empty_block_key, non_empty_block);

    let mut drop_tile = Tile::new();
    drop_tile.set(&empty_block_key, Block::new());

    let mut bitmap = JourneyBitmap::of_tile_bytes_without_validation(vec![
        (keep_key, keep_tile.serialize()),
        (drop_key, drop_tile.serialize()),
    ])
    .unwrap();

    bitmap.edit().validate().unwrap();

    assert!(bitmap.contains_tile(&keep_key));
    assert!(!bitmap.contains_tile(&drop_key));

    let keep_tile = bitmap.get_tile(&keep_key).expect("tile should remain");
    assert!(keep_tile.get(&empty_block_key).is_none());
    assert!(keep_tile.get(&non_empty_block_key).is_some());
}

#[test]
fn sub_kilometre_arc_entering_and_leaving_a_cap_is_not_lost() {
    use memolanes_core::journey_bitmap::MERCATOR_MAX_LATITUDE;
    for sign in [1.0, -1.0] {
        let latitude = sign * (MERCATOR_MAX_LATITUDE - 0.000001);
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(-0.05, latitude, 0.05, latitude);
        let plane = if sign > 0.0 {
            BitmapPlane::NorthPolar
        } else {
            BitmapPlane::SouthPolar
        };
        assert!(
            bitmap.has_plane_data(plane),
            "the short arc's interior crosses the seam twice"
        );
        assert!(bitmap.has_plane_data(BitmapPlane::Mercator));
        let mut reversed = JourneyBitmap::new();
        reversed.edit().add_line(0.05, latitude, -0.05, latitude);
        assert_eq!(bitmap, reversed);
    }
}

#[test]
fn set_operations_keep_same_coordinates_in_different_planes_independent() {
    let bitmap_with = |entries: &[(BitmapPlane, &[u8])]| {
        let mut bitmap = JourneyBitmap::new();
        for &(plane, points) in entries {
            let mut block = Block::new();
            for &x in points {
                block.set_point(x, 0, true);
            }
            let mut tile = Tile::new();
            tile.set(&BlockKey::from_x_y(0, 0), block);
            bitmap
                .edit()
                .insert_tile(&TileKey::new(plane, 64, 64), tile);
        }
        bitmap
    };
    use BitmapPlane::{Mercator, NorthPolar, SouthPolar};
    let mut left = bitmap_with(&[(Mercator, &[1]), (NorthPolar, &[1, 2])]);
    let right = bitmap_with(&[(NorthPolar, &[2, 3]), (SouthPolar, &[1])]);
    // Exercise both lazy and decoded operands.
    let north_key = TileKey::NorthPolar { x: 64, y: 64 };
    let bytes = left.get_tile_bytes(&north_key).unwrap().into_owned();
    left.edit().insert_tile_bytes(north_key, bytes).unwrap();

    let expected_union = bitmap_with(&[
        (Mercator, &[1]),
        (NorthPolar, &[1, 2, 3]),
        (SouthPolar, &[1]),
    ]);
    let mut union = left.clone();
    let mut editor = union.edit();
    editor.merge(right.clone());
    assert!(editor.finish().is_empty());
    assert_eq!(union, expected_union);
    let mut cloned_union = left.clone();
    cloned_union.edit().merge_with_partial_clone(&right);
    assert_eq!(cloned_union, expected_union);

    let mut intersection = left.clone();
    let mut editor = intersection.edit_tracked();
    editor.intersection(&right);
    let changed_tiles = editor.finish();
    assert_eq!(intersection, bitmap_with(&[(NorthPolar, &[2])]));
    assert!(changed_tiles.contains(&TileKey::Mercator { x: 64, y: 64 }));

    left.edit().difference(&right);
    assert_eq!(left, bitmap_with(&[(Mercator, &[1]), (NorthPolar, &[1])]));
    for bitmap in [&mut union, &mut cloned_union, &mut intersection, &mut left] {
        bitmap.check_invariant_and_debug_log();
    }
}
