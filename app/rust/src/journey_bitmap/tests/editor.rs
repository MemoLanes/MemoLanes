use super::*;
use crate::journey_bitmap::{BITMAP_WIDTH, TILE_WIDTH};
use bitvec::prelude::*;
use std::collections::BTreeSet;

fn mercator_pixels(bitmap: &JourneyBitmap) -> BTreeSet<(i64, i64)> {
    let mut pixels = BTreeSet::new();
    for key in bitmap.all_tile_keys() {
        let TileKey::Mercator { x, y } = *key else {
            panic!("expected Mercator test coverage, got {key:?}");
        };
        bitmap.peek_tile_without_updating_cache(key, |tile| {
            let tile = tile.unwrap();
            for (block_key, block) in tile.iter() {
                let origin_x = (x as i64 * TILE_WIDTH + block_key.x() as i64) * BITMAP_WIDTH;
                let origin_y = (y as i64 * TILE_WIDTH + block_key.y() as i64) * BITMAP_WIDTH;
                for i in block.raw_data().view_bits::<Msb0>().iter_ones() {
                    pixels.insert((
                        origin_x + i as i64 % BITMAP_WIDTH,
                        origin_y + i as i64 / BITMAP_WIDTH,
                    ));
                }
            }
        });
    }
    pixels
}

#[test]
fn square_strokes_cover_the_full_brush_across_block_and_tile_boundaries() {
    let tile_side = 1 << ALL_OFFSET;
    for width in [1_u8, 4, 9] {
        for (dx, dy) in [
            (0_i64, 0_i64),
            (12, 0),
            (0, 12),
            (128, 0),
            (0, 128),
            (12, 12),
            (12, -12),
            (12, 5),
            (5, 12),
            (12, -5),
            (5, -12),
            (-12, 5),
            (-5, 12),
            (-12, -5),
            (-5, -12),
        ] {
            for start in [
                (32, 32),
                (BITMAP_WIDTH, BITMAP_WIDTH),
                (tile_side - 1, tile_side - 1),
                (tile_side, tile_side),
                (tile_side + 1, tile_side + 1),
            ] {
                let end = (start.0 + dx, start.1 + dy);
                let (a, b) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                let steps = dx.abs().max(dy.abs());
                let mut expected = BTreeSet::new();
                // Independent oracle: round points sampled uniformly along
                // the dominant axis, then stamp each *complete* square.
                for i in 0..=steps {
                    let t = if steps == 0 {
                        0.0
                    } else {
                        i as f64 / steps as f64
                    };
                    let x = a.0 + ((b.0 - a.0) as f64 * t).round() as i64;
                    let y = a.1 + ((b.1 - a.1) as f64 * t).round() as i64;
                    let before = i64::from(width) / 2;
                    for x in x - before..x - before + i64::from(width) {
                        for y in y - before..y - before + i64::from(width) {
                            expected.insert((x, y));
                        }
                    }
                }
                let mut plane = JourneyBitmap::new();
                let mut editor = plane.edit_tracked();
                editor.draw_line(BitmapPlane::Mercator, start, end, width);
                let changed_tiles = editor.finish();
                assert_eq!(
                    mercator_pixels(&plane),
                    expected,
                    "{start:?}->{end:?}, width={width}"
                );
                assert_eq!(changed_tiles.len(), plane.tile_count());

                // Repeating/reversing a stroke must not invalidate render caches.
                let mut editor = plane.edit_tracked();
                editor.draw_line(BitmapPlane::Mercator, end, start, width);
                assert!(editor.finish().is_empty());
            }
        }
    }
}

#[test]
fn square_brush_clips_at_plane_edges_without_wrapping_or_empty_tiles() {
    let mut plane = JourneyBitmap::new();
    let last = (BitmapPlane::Mercator.grid_width() << ALL_OFFSET) - 1;
    for x in [0, last] {
        for y in [0, last] {
            plane
                .edit()
                .draw_line(BitmapPlane::Mercator, (x, y), (x, y), 9);
        }
    }
    let pixels = mercator_pixels(&plane);
    assert_eq!(pixels.len(), 4 * 25);
    assert!(pixels
        .iter()
        .all(|&(x, y)| { (x <= 4 || x >= last - 4) && (y <= 4 || y >= last - 4) }));
    assert_eq!(plane.tile_count(), 4);
}

#[test]
fn ordinary_short_segments_use_direct_mercator_rasterization() {
    for (lng0, lat0, lng1, lat1) in [
        (0.0, 0.0, 0.0002, 0.0002),
        (151.1, -33.8, 151.1001, -33.8001),
        (114.2, 22.6, 114.2, 22.6),
        (179.99999, 54.0, -179.99999, 54.00001),
        (30.0, 84.0, 30.0001, 84.0001),
        (539.99999, -54.0, -179.99999, -54.00001),
        (0.0, 0.0, 0.004, 0.004),
        (151.1, -33.8, 151.105, -33.804),
        (114.2, 22.6, 114.204, 22.604),
        (179.999, 54.0, -179.999, 54.001),
        (30.0, 71.0, 30.004, 71.004),
    ] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.edit().add_line(lng0, lat0, lng1, lat1);
        let mut legacy = JourneyBitmap::new();
        legacy.edit().add_mercator_line(lng0, lat0, lng1, lat1);
        assert_eq!(bitmap, legacy);
        let mut reversed = JourneyBitmap::new();
        reversed.edit().add_line(lng1, lat1, lng0, lat0);
        assert_eq!(bitmap, reversed);
    }
}

#[test]
fn wide_mercator_tracks_keep_complete_brushes_at_blocks_tiles_and_dateline() {
    let n = MERCATOR_WORLD_PIXEL_WIDTH;
    let width_at_y = |y| {
        let latitude = utils::tile_y_to_lat(y as i32, MERCATOR_WORLD_PIXEL_OFFSET as i32);
        (1.0 / latitude.to_radians().cos()).round() as i64
    };
    let transition_y =
        ((1.0 - 2.5_f64.acosh() / std::f64::consts::PI) * n as f64 / 2.0).floor() as i64;
    for start_x in [48, (1 << ALL_OFFSET) - 8, n - 8] {
        // Even/odd widths in both hemispheres, plus the width 3 -> 2 boundary.
        for start_y in [256, n - 400, n / 4, transition_y - 68] {
            for (dx, dy) in [
                (17, 137),
                (17, -137),
                (137, 17),
                (137, -17),
                (0, 137),
                (137, 0),
                (0, 0),
            ] {
                let (end_x, end_y) = (start_x + dx, start_y + dy);
                let (lng0, lat0) = mercator_cell_to_lng_lat(start_x, start_y);
                let (lng1, lat1) = mercator_cell_to_lng_lat(end_x, end_y);
                let mut bitmap = JourneyBitmap::new();
                let mut editor = bitmap.edit_tracked();
                editor.add_line(lng0, lat0, lng1, lat1);
                let changed = editor.finish();
                assert_eq!(changed, bitmap.all_tile_keys().copied().collect());

                // Independent oracle: sample the dominant axis and stamp complete
                // perpendicular spans, with caps only at the segment endpoints.
                // Storage boundaries must neither clip spans nor restart caps.
                let mut expected = BTreeSet::new();
                let steps = dx.abs().max(dy.abs());
                for i in 0..=steps {
                    let t = if steps == 0 {
                        0.0
                    } else {
                        i as f64 / steps as f64
                    };
                    let x = start_x + (dx as f64 * t).round() as i64;
                    let y = start_y + (dy as f64 * t).round() as i64;
                    let width = width_at_y(y);
                    if dx >= dy.abs() {
                        for y in y - width / 2..y - width / 2 + width {
                            expected.insert((x.rem_euclid(n), y));
                        }
                    } else {
                        for x in x - width / 2..x - width / 2 + width {
                            expected.insert((x.rem_euclid(n), y));
                        }
                    }
                }
                for (x, y) in [(start_x, start_y), (end_x, end_y)] {
                    let width = width_at_y(y);
                    for x in x - width / 2..x - width / 2 + width {
                        for y in y - width / 2..y - width / 2 + width {
                            expected.insert((x.rem_euclid(n), y));
                        }
                    }
                }
                assert_eq!(
                    mercator_pixels(&bitmap),
                    expected,
                    "({start_x}, {start_y}) + ({dx}, {dy})"
                );
                let mut reverse = JourneyBitmap::new();
                reverse.edit().add_line(lng1, lat1, lng0, lat0);
                assert_eq!(bitmap, reverse);
                let mut editor = bitmap.edit_tracked();
                editor.add_line(lng1, lat1, lng0, lat0);
                assert!(editor.finish().is_empty());
            }
        }
    }
}

fn mercator_cell_to_lng_lat(x: i64, y: i64) -> (f64, f64) {
    let n = MERCATOR_WORLD_PIXEL_WIDTH;
    // Stay off integer projection boundaries so inverse roundoff cannot
    // change the source cell selected by the public geographic API.
    let u = (x as f64 + 0.25) / n as f64;
    let v = (y as f64 + 0.25) / n as f64;
    (
        u * 360.0 - 180.0,
        (std::f64::consts::PI * (1.0 - 2.0 * v))
            .sinh()
            .atan()
            .to_degrees(),
    )
}

#[test]
fn single_pixel_strokes_include_endpoints_in_the_next_partition() {
    let n = MERCATOR_WORLD_PIXEL_WIDTH;
    let tile_side = 1 << ALL_OFFSET;
    // End exactly in the next block/tile, including the wrapped world edge.
    for end_x in [n / 2 + BITMAP_WIDTH, n / 2 + tile_side, n] {
        for end_y in [n / 2 + BITMAP_WIDTH, n / 2 + tile_side] {
            for (dx, dy) in [
                (11_i64, 7_i64),
                (7, 11),
                (11, -7),
                (7, -11),
                (11, 0),
                (0, 11),
            ] {
                let (start_x, start_y) = (end_x - dx, end_y - dy);
                let a = mercator_cell_to_lng_lat(start_x, start_y);
                let b = mercator_cell_to_lng_lat(end_x, end_y);
                let steps = dx.abs().max(dy.abs());
                let expected = (0..=steps)
                    .map(|i| {
                        let t = i as f64 / steps as f64;
                        (
                            (start_x + (dx as f64 * t).round() as i64).rem_euclid(n),
                            start_y + (dy as f64 * t).round() as i64,
                        )
                    })
                    .collect::<BTreeSet<_>>();
                let mut bitmap = JourneyBitmap::new();
                let mut editor = bitmap.edit_tracked();
                editor.add_line(a.0, a.1, b.0, b.1);
                let changed = editor.finish();
                assert_eq!(
                    mercator_pixels(&bitmap),
                    expected,
                    "({start_x}, {start_y}) -> ({end_x}, {end_y})"
                );
                assert_eq!(changed, bitmap.all_tile_keys().copied().collect());
                let mut reverse = JourneyBitmap::new();
                reverse.edit().add_line(b.0, b.1, a.0, a.1);
                assert_eq!(bitmap, reverse);
                let mut editor = bitmap.edit_tracked();
                editor.add_line(b.0, b.1, a.0, a.1);
                assert!(editor.finish().is_empty());
            }
        }
    }
}
