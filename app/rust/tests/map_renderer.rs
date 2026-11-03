pub mod test_utils;
use memolanes_core::{journey_bitmap::JourneyBitmap, renderer::*, utils::lng_lat_to_tile_x_y};

#[macro_use]
extern crate assert_float_eq;

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;

#[test]
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap
        .edit()
        .add_line(START_LNG, START_LAT, END_LNG, END_LAT);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = test_utils::render_map_overlay(
        &mut map_renderer,
        11,
        START_LNG,
        START_LAT,
        END_LNG,
        END_LAT,
    );
    assert_f64_near!(render_result.left, 150.8203125);
    assert_f64_near!(render_result.top, -33.578014746143985);
    assert_f64_near!(render_result.right, 151.5234375);
    assert_f64_near!(render_result.bottom, -34.16181816123038);

    test_utils::verify_image("map_renderer_basic", &render_result.data);
}

#[test]
fn no_op_update_preserves_version_and_real_update_invalidates_it() {
    let mut renderer = MapRenderer::new(JourneyBitmap::new());
    renderer.update(|editor| {
        editor.add_line(START_LNG, START_LAT, END_LNG, END_LAT);
    });
    let version = renderer.get_current_version();
    let version_string = renderer.get_version_string();
    let area = renderer.get_current_area();

    renderer.update(|editor| {
        editor.add_line(START_LNG, START_LAT, END_LNG, END_LAT);
    });

    assert_eq!(renderer.get_current_version(), version);
    assert!(renderer.matches_version(Some(&version_string)));
    assert_eq!(renderer.get_current_area(), area);

    renderer.update(|editor| {
        editor.add_line(START_LNG + 1.0, START_LAT, END_LNG + 1.0, END_LAT);
    });

    assert_eq!(renderer.get_current_version(), version.wrapping_add(1));
    assert!(!renderer.matches_version(Some(&version_string)));
    assert!(renderer.get_current_area() > area);
}

#[test]
fn validation_invalidates_cached_block_occupancy_for_lazy_and_decoded_tiles() {
    use memolanes_core::journey_bitmap::{Block, BlockKey, Tile, TileKey};

    let key = TileKey::mercator(300, 200);
    let query = |renderer: &mut MapRenderer| {
        renderer
            .get_tile_range_response(300, 200, 9, 1, 1, 6)
            .unwrap()
    };
    for lazy in [false, true] {
        let mut tile = Tile::new();
        tile.set(&BlockKey::from_x_y(0, 0), Block::new());
        let mut block = Block::new();
        block.set_point(63, 63, true);
        tile.set(&BlockKey::from_x_y(127, 127), block);
        let mut bitmap = JourneyBitmap::new();
        if lazy {
            bitmap
                .edit()
                .insert_tile_bytes(key, tile.serialize())
                .unwrap();
        } else {
            bitmap.edit().insert_tile(&key, tile);
        }
        let mut renderer = MapRenderer::new(bitmap);
        // Coarse rendering caches block presence without decoding lazy tiles.
        let before = query(&mut renderer);
        let version = renderer.get_current_version();

        renderer.update(|editor| editor.validate().unwrap());

        assert_eq!(renderer.get_current_version(), version + 1, "lazy={lazy}");
        let after = query(&mut renderer);
        let mut fresh = MapRenderer::new(renderer.peek_latest_bitmap().clone());
        assert_ne!(after, before, "lazy={lazy}");
        assert_eq!(after, query(&mut fresh), "lazy={lazy}");

        // A second validation makes no edits and must preserve the version.
        renderer.update(|editor| editor.validate().unwrap());
        assert_eq!(renderer.get_current_version(), version + 1, "lazy={lazy}");
        assert_eq!(query(&mut renderer), after, "lazy={lazy}");
    }
}

#[test]
fn scoped_edits_invalidate_cached_tiles_and_area_across_mutation_paths() {
    use memolanes_core::journey_bitmap::{Block, BlockKey, Tile, TileKey};

    let block_key = BlockKey::from_x_y(0, 0);
    let tile_with = |points: &[u8]| {
        let mut block = Block::new();
        for &x in points {
            block.set_point(x, 0, true);
        }
        let mut tile = Tile::new();
        tile.set(&block_key, block);
        tile
    };
    let bitmap_with = |entries: &[(TileKey, &[u8])]| {
        let mut bitmap = JourneyBitmap::new();
        for &(key, points) in entries {
            bitmap.edit().insert_tile(&key, tile_with(points));
        }
        bitmap
    };
    let first = TileKey::mercator(300, 200);
    let second = TileKey::mercator(301, 200);
    let third = TileKey::mercator(302, 200);
    let mut renderer = MapRenderer::new(bitmap_with(&[(first, &[0])]));
    let partial_merge = bitmap_with(&[(first, &[32]), (second, &[0])]);
    let keep = bitmap_with(&[(first, &[32])]);
    let expected: &[&[(TileKey, &[u8])]] = &[
        &[(first, &[16])],
        &[(first, &[16, 32]), (second, &[0])],
        &[(first, &[32, 48]), (second, &[0])],
        &[(first, &[32, 48]), (second, &[0]), (third, &[0])],
        &[(first, &[32])],
        &[],
    ];
    let query = |renderer: &mut MapRenderer, key: TileKey| {
        let TileKey::Mercator { x, y } = key else {
            panic!("rendering test requires a Mercator tile, got {key:?}");
        };
        renderer
            .get_tile_range_response(x.into(), y.into(), 9, 1, 1, 10)
            .unwrap()
    };
    // Prime both populated and empty cache entries before any edit. Each round
    // then leaves the caches warm for the next, including after tile deletion.
    renderer.get_current_area();
    for key in [first, second, third] {
        query(&mut renderer, key);
    }

    for (step, entries) in expected.iter().enumerate() {
        let version = renderer.get_current_version();
        renderer.update(|editor| match step {
            0 => editor
                .insert_tile_bytes(first, tile_with(&[16]).serialize())
                .unwrap(),
            1 => editor.merge_with_partial_clone(&partial_merge),
            2 => {
                let (_, block) = editor
                    .get_tile_mut(&first)
                    .unwrap()
                    .iter_mut()
                    .next()
                    .unwrap();
                block.set_point(16, 0, false);
                block.set_point(48, 0, true);
            }
            3 => *editor.get_tile_mut_or_insert_empty(&third) = tile_with(&[0]),
            4 => editor.intersection(&keep),
            5 => editor.difference(&keep),
            _ => unreachable!(),
        });

        let expected_bitmap = bitmap_with(entries);
        assert_eq!(renderer.peek_latest_bitmap(), &expected_bitmap);
        assert_eq!(renderer.get_current_version(), version.wrapping_add(1));
        let mut fresh = MapRenderer::new(expected_bitmap);
        assert_eq!(renderer.get_current_area(), fresh.get_current_area());
        for key in [first, second, third] {
            assert_eq!(
                query(&mut renderer, key),
                query(&mut fresh, key),
                "step {step}, {key:?}"
            );
        }
    }
}

#[test]
fn source_update_changes_only_the_overlapping_rendered_tile() {
    let lng = 114.2;
    let lat = 22.7;
    let (source_x, source_y) = lng_lat_to_tile_x_y(lng, lat, 9);
    let adjacent_x = (source_x + 2).min(511);
    let mut renderer = MapRenderer::new(JourneyBitmap::new());

    let source_before = renderer
        .get_tile_range_response(i64::from(source_x), i64::from(source_y), 9, 1, 1, 6)
        .unwrap();
    let adjacent_before = renderer
        .get_tile_range_response(i64::from(adjacent_x), i64::from(source_y), 9, 1, 1, 6)
        .unwrap();

    renderer.update(|editor| {
        editor.add_line(lng, lat, lng + 0.0001, lat + 0.0001);
    });

    let adjacent_after = renderer
        .get_tile_range_response(i64::from(adjacent_x), i64::from(source_y), 9, 1, 1, 6)
        .unwrap();
    let source_after = renderer
        .get_tile_range_response(i64::from(source_x), i64::from(source_y), 9, 1, 1, 6)
        .unwrap();

    assert_eq!(adjacent_after, adjacent_before);
    assert_ne!(source_after, source_before);
}

#[test]
fn source_update_changes_overlapping_fine_and_coarse_rendered_tiles() {
    let lng = 114.2;
    let lat = 22.7;
    let (source_x, source_y) = lng_lat_to_tile_x_y(lng, lat, 9);
    let (fine_x, fine_y) = lng_lat_to_tile_x_y(lng, lat, 10);
    let coarse_x = source_x >> 1;
    let coarse_y = source_y >> 1;
    let unrelated_source_x = (source_x + 2).min(511);
    let unrelated_fine_x = unrelated_source_x * 2;
    let unrelated_fine_y = source_y * 2;
    let mut renderer = MapRenderer::new(JourneyBitmap::new());

    let fine_before = renderer
        .get_tile_range_response(i64::from(fine_x), i64::from(fine_y), 10, 1, 1, 6)
        .unwrap();
    let coarse_before = renderer
        .get_tile_range_response(i64::from(coarse_x), i64::from(coarse_y), 8, 1, 1, 6)
        .unwrap();
    let unrelated_before = renderer
        .get_tile_range_response(
            i64::from(unrelated_fine_x),
            i64::from(unrelated_fine_y),
            10,
            1,
            1,
            6,
        )
        .unwrap();

    renderer.update(|editor| {
        editor.add_line(lng, lat, lng + 0.0001, lat + 0.0001);
    });

    let unrelated_after = renderer
        .get_tile_range_response(
            i64::from(unrelated_fine_x),
            i64::from(unrelated_fine_y),
            10,
            1,
            1,
            6,
        )
        .unwrap();
    let fine_after = renderer
        .get_tile_range_response(i64::from(fine_x), i64::from(fine_y), 10, 1, 1, 6)
        .unwrap();
    let coarse_after = renderer
        .get_tile_range_response(i64::from(coarse_x), i64::from(coarse_y), 8, 1, 1, 6)
        .unwrap();

    assert_eq!(unrelated_after, unrelated_before);
    assert_ne!(fine_after, fine_before);
    assert_ne!(coarse_after, coarse_before);
}

#[test]
fn replace_discards_cached_content_and_empty_results_at_all_resolutions() {
    let mut source = JourneyBitmap::new();
    source.edit().add_line(114.2, 22.7, 114.21, 22.71);
    let mut renderer = MapRenderer::new(source.clone());
    let query = |renderer: &mut MapRenderer, zoom: i16| {
        let (x, y) = lng_lat_to_tile_x_y(114.2, 22.7, i32::from(zoom));
        renderer
            .get_tile_range_response(i64::from(x), i64::from(y), zoom, 2, 1, 6)
            .unwrap()
    };
    for zoom in [8, 9, 10] {
        query(&mut renderer, zoom);
    }

    // Exercise both non-empty -> empty and cached empty -> non-empty.
    for replacement in [JourneyBitmap::new(), source] {
        let old_version = renderer.get_version_string();
        renderer.replace(replacement.clone());
        assert!(!renderer.matches_version(Some(&old_version)));
        let mut fresh = MapRenderer::new(replacement);
        for zoom in [8, 9, 10] {
            let expected = query(&mut fresh, zoom);
            assert_eq!(query(&mut renderer, zoom), expected);
            assert_eq!(query(&mut renderer, zoom), expected);
        }
    }
}

#[test]
fn wrapped_world_requests_preserve_coordinates_and_see_source_updates() {
    use journey_kernel::tile_range::{decode_tile_range_response, parse_tile_range_header};

    let (lng, lat) = (-179.8, 22.7);
    let (x, y) = lng_lat_to_tile_x_y(lng, lat, 9);
    let mut source = JourneyBitmap::new();
    source.edit().add_line(lng, lat, lng + 0.01, lat + 0.01);
    let mut renderer = MapRenderer::new(source);

    // Start with cached empty results, then invalidate them while recording.
    let mut empty_renderer = MapRenderer::new(JourneyBitmap::new());
    for world_offset in [-512_i64, 0, 512] {
        empty_renderer
            .get_tile_range_response(i64::from(x) + world_offset, i64::from(y), 9, 2, 1, 6)
            .unwrap();
    }
    empty_renderer.update(|editor| {
        editor.add_line(lng, lat, lng + 0.01, lat + 0.01);
    });

    let reference = renderer
        .get_tile_range_response(i64::from(x), i64::from(y), 9, 2, 1, 6)
        .unwrap();
    for world_offset in [-512_i64, 0, 512] {
        let request_x = i64::from(x) + world_offset;
        let response = empty_renderer
            .get_tile_range_response(request_x, i64::from(y), 9, 2, 1, 6)
            .unwrap();
        let header = parse_tile_range_header(&response).unwrap();
        let tiles = decode_tile_range_response(&response).unwrap();
        assert_eq!(header.x0, request_x as i32);
        assert_eq!(header.tile_count, 2);
        assert_eq!(header.present_count, 1);
        assert_eq!(tiles.len(), 1);
        assert_eq!((tiles[0].0, tiles[0].1), (request_x as i32, y as i32));

        // Only the origin differs between world copies; the entire encoded
        // bitmap/LOD payload must agree, including the absent neighboring tile.
        let mut normalized = response;
        normalized[4..8].copy_from_slice(&(x as i32).to_le_bytes());
        assert_eq!(normalized, reference);
    }
}

#[test]
fn rasterized_tile_ranges_match_snapshot() {
    use journey_kernel::tile_range::decompress_tile_range_response;
    use sha2::{Digest, Sha256};

    // Hash the spherical add_line output through the existing renderer. Cover
    // base pixels and every LOD independently of compression, including source zoom boundaries,
    // coarse occupancy, fine pixels, empty regions and the antimeridian.
    let mut source = JourneyBitmap::new();
    for (x0, y0, x1, y1) in [
        (114.10, 22.60, 114.35, 22.82),
        (114.12, 22.82, 114.36, 22.61),
        (-179.99, 22.70, -179.70, 22.71),
        (179.99, 22.70, -179.99, 22.71),
    ] {
        source.edit().add_line(x0, y0, x1, y1);
    }
    let mut renderer = MapRenderer::new(source);
    for (zoom, expected) in [
        (
            0_i16,
            "3d5eaf59bcb465e10106d4fb3f3005dbf5156026743118b2a83f390db1f6ba25",
        ),
        (
            8_i16,
            "1f98b05c4ef27271b0aef76b5116822d792422291fa4eacae4f01ec1c1852179",
        ),
        (
            9_i16,
            "e14964ad667b9c2bb6ab3c0da66a3e674faebf53a99c969e50edd5601cb5a37c",
        ),
        (
            16_i16,
            "7a29c6e3d61c806f5f1bfe8dc7476593655d9972f0d872f38048a383c4f221ce",
        ),
    ] {
        let mut digest = Sha256::new();
        for power in 6..=11 {
            for (lng, lat) in [(114.2, 22.7), (-179.99, 22.7), (0.0, 0.0)] {
                let (x, y) = lng_lat_to_tile_x_y(lng, lat, zoom.into());
                let response = renderer
                    .get_tile_range_response(x.into(), y.into(), zoom, 2, 2, power)
                    .unwrap();
                let hot = renderer
                    .get_tile_range_response(x.into(), y.into(), zoom, 2, 2, power)
                    .unwrap();
                assert_eq!(response, hot);
                digest.update(decompress_tile_range_response(&response).unwrap());
            }
        }
        assert_eq!(hex::encode(digest.finalize()), expected, "zoom {zoom}");
    }
}

#[test]
fn polar_complex_gpx_renders_both_caps() {
    use image::{Rgba, RgbaImage};
    use journey_kernel::bitmap2d::BitMap2D;
    use journey_kernel::tile_range::decode_tile_range_response;
    use memolanes_core::{import_data, journey_bitmap::BitmapPlane};
    use std::io::Cursor;

    let (raw_data, _) = import_data::gpx::load_gpx("./tests/data/polar_complex.gpx").unwrap();
    let vector = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data, None,
    )
    .unwrap();
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(&vector);
    let mut renderer = MapRenderer::new(bitmap);

    // The compact fixture's roses are only 600 m across. Render the four
    // regional tiles surrounding each pole so the image actually resolves the
    // petals, rather than reducing each whole cap to a single occupied pixel.
    const SIDE: u32 = 1024;
    let mut image = RgbaImage::new(SIDE * 2, SIDE);
    for (index, plane) in [BitmapPlane::NorthPolar, BitmapPlane::SouthPolar]
        .into_iter()
        .enumerate()
    {
        let response = renderer
            .get_plane_tile_range_response(plane, 511, 511, 10, 2, 2, 9)
            .unwrap();
        let tiles = decode_tile_range_response(&response).unwrap();
        assert!(!tiles.is_empty(), "{plane:?} must contain rendered tracks");
        let mut cap = BitMap2D::new(10);
        for (tile_x, tile_y, tile) in tiles {
            for y in 0..tile.side() {
                for x in 0..tile.side() {
                    if tile.get(x, y) {
                        cap.set(
                            (tile_x - 511) as usize * tile.side() + x,
                            (tile_y - 511) as usize * tile.side() + y,
                            true,
                        );
                    }
                }
            }
        }
        assert!(
            cap.get(512, 512),
            "{plane:?} must include the pole crossings"
        );

        for y in 0..SIDE {
            for x in 0..SIDE {
                let color = if cap.get(x as usize, y as usize) {
                    Rgba([0, 100, 160, 255])
                } else {
                    Rgba([255, 255, 255, 255])
                };
                image.put_pixel(index as u32 * SIDE + x, y, color);
            }
        }
    }

    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    test_utils::verify_image("map_renderer_polar_complex", &png);
}

#[test]
fn polar_updates_invalidate_versions_and_area_without_changing_mercator_tiles() {
    use memolanes_core::journey_bitmap::BitmapPlane;
    for (latitude, plane) in [
        (89.0, BitmapPlane::NorthPolar),
        (-89.0, BitmapPlane::SouthPolar),
    ] {
        let mut renderer = MapRenderer::new(JourneyBitmap::new());
        let mercator = renderer.get_tile_range_response(0, 0, 0, 1, 1, 6).unwrap();
        let empty_cap = renderer
            .get_plane_tile_range_response(plane, 0, 0, 0, 1, 1, 6)
            .unwrap();
        assert_eq!(renderer.get_current_area(), 0);
        renderer.update(|editor| {
            editor.add_line(10.0, latitude, 11.0, latitude);
        });
        assert_eq!(renderer.get_current_version(), 1);
        let area = renderer.get_current_area();
        assert!(area > 0);
        let cap = renderer
            .get_plane_tile_range_response(plane, 0, 0, 0, 1, 1, 6)
            .unwrap();
        assert_ne!(cap, empty_cap);
        assert_eq!(
            renderer.get_tile_range_response(0, 0, 0, 1, 1, 6).unwrap(),
            mercator
        );

        renderer.update(|editor| {
            editor.add_line(10.0, latitude, 11.0, latitude);
        });
        assert_eq!(
            renderer.get_current_version(),
            1,
            "duplicate polar fixes are no-ops"
        );
        assert_eq!(renderer.get_current_area(), area);
        assert_eq!(
            renderer
                .get_plane_tile_range_response(plane, 0, 0, 0, 1, 1, 6)
                .unwrap(),
            cap
        );

        renderer.update(|editor| {
            editor.add_line(40.0, latitude, 50.0, latitude);
        });
        assert_eq!(renderer.get_current_version(), 2);
        assert!(renderer.get_current_area() > area);
        assert_ne!(
            renderer
                .get_plane_tile_range_response(plane, 0, 0, 0, 1, 1, 6)
                .unwrap(),
            cap
        );
    }
}

#[test]
fn plane_versions_and_render_cache_follow_plane_qualified_writes() {
    use memolanes_core::journey_bitmap::BitmapPlane;
    let mut renderer = MapRenderer::new(JourneyBitmap::new());
    let mercator = renderer.get_plane_version_string(BitmapPlane::Mercator);
    let north = renderer.get_plane_version_string(BitmapPlane::NorthPolar);
    renderer.update(|editor| editor.add_line(45.0, -88.0, 45.01, -88.01));
    assert_eq!(
        renderer.get_plane_version_string(BitmapPlane::Mercator),
        mercator
    );
    assert_eq!(
        renderer.get_plane_version_string(BitmapPlane::NorthPolar),
        north
    );
    assert!(!renderer.matches_plane_version(BitmapPlane::SouthPolar, Some("0")));
    let cap = renderer
        .get_plane_tile_range_response(BitmapPlane::NorthPolar, 0, 0, 0, 1, 1, 6)
        .unwrap();
    let mut delta = JourneyBitmap::new();
    delta.edit().add_line(0.0, 89.0, 0.01, 89.01);
    renderer.update(|editor| editor.merge(delta));
    assert_ne!(
        renderer
            .get_plane_tile_range_response(BitmapPlane::NorthPolar, 0, 0, 0, 1, 1, 6)
            .unwrap(),
        cap,
        "merge must invalidate even a previously empty cached cap"
    );
    assert_eq!(
        renderer.get_plane_version_string(BitmapPlane::Mercator),
        mercator
    );
    assert!(renderer
        .get_plane_tile_range_response(BitmapPlane::NorthPolar, -1, 0, 1, 1, 1, 6)
        .is_err());
    assert!(renderer
        .get_plane_tile_range_response(BitmapPlane::NorthPolar, 1, 1, 1, 2, 2, 6)
        .is_err());
}

#[test]
fn regional_polar_tiles_preserve_native_pixels() {
    use journey_kernel::tile_range::decode_tile_range_response;
    use memolanes_core::journey_bitmap::{BitmapPlane, MERCATOR_MAX_LATITUDE};
    for plane in [BitmapPlane::NorthPolar, BitmapPlane::SouthPolar] {
        let sign = if plane == BitmapPlane::NorthPolar {
            1.0
        } else {
            -1.0
        };
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(90.0, sign * 88.0, 90.0, sign * 88.0);
        let mut renderer = MapRenderer::new(bitmap);
        let radius = (1.0_f64.to_radians()).sin()
            / ((90.0 - MERCATOR_MAX_LATITUDE).to_radians() / 2.0).sin();
        let native_x = ((0.5 + 0.5 * radius) * (1 << 20) as f64).round() as usize;
        let native_y = 1 << 19;
        let x = (native_x / 1024) as i64;
        let y = (native_y / 1024) as i64;
        let response = renderer
            .get_plane_tile_range_response(plane, x, y, 10, 1, 1, 10)
            .unwrap();
        let tiles = decode_tile_range_response(&response).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!((tiles[0].0, tiles[0].1), (x as i32, y as i32));
        assert!(
            tiles[0].2.get(native_x % 1024, native_y % 1024),
            "fine polar tiles must retain the original metre-scale pixel"
        );
    }
}

#[test]
fn equal_tile_coordinates_in_different_planes_keep_independent_caches() {
    use journey_kernel::tile_range::decode_tile_range_response;
    use memolanes_core::journey_bitmap::{BitmapPlane, Block, BlockKey, Tile, TileKey};

    let mut renderer = MapRenderer::new(JourneyBitmap::new());
    let query = |renderer: &mut MapRenderer, plane: BitmapPlane| {
        renderer
            .get_plane_tile_range_response(plane, 64, 64, plane.source_tile_zoom(), 1, 1, 10)
            .unwrap()
    };
    let mut responses = BitmapPlane::ALL.map(|plane| query(&mut renderer, plane));
    for (index, plane) in BitmapPlane::ALL.into_iter().enumerate() {
        let versions = BitmapPlane::ALL.map(|p| renderer.get_plane_version_string(p));
        let mut block = Block::new();
        block.set_point(8 + index as u8 * 16, 16, true);
        let mut tile = Tile::new();
        tile.set(&BlockKey::from_x_y(0, 0), block);
        renderer.update(|editor| editor.insert_tile(&TileKey::new(plane, 64, 64), tile));

        for (other_index, other) in BitmapPlane::ALL.into_iter().enumerate() {
            let response = query(&mut renderer, other);
            if other == plane {
                assert_ne!(
                    renderer.get_plane_version_string(other),
                    versions[other_index]
                );
                let decoded = decode_tile_range_response(&response).unwrap();
                assert_eq!(decoded.len(), 1);
                assert!(decoded[0].2.get(1 + index * 2, 2));
                assert_ne!(response, responses[other_index]);
            } else {
                assert_eq!(
                    renderer.get_plane_version_string(other),
                    versions[other_index]
                );
                assert_eq!(response, responses[other_index]);
            }
            responses[other_index] = response;
        }
    }
}
