use journey_kernel::bitmap2d::BitMap2D;

use super::SOURCE_TILE_ZOOM;
use crate::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey};
use crate::journey_bitmap::{BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, TILE_WIDTH_OFFSET};

/// Maps a selected part of a source tile or block into the output bitmap.
#[derive(Clone, Copy)]
struct RasterRegion {
    /// Top-left output pixel, before clipping.
    origin: (i64, i64),
    /// Source subdivision selected from a 2^zoom_factor × 2^zoom_factor grid.
    sub_index: (i64, i64),
    zoom_factor: i16,
    /// Output side-length exponent; zero or negative means at most one pixel.
    size_power: i16,
}

/// Integer scaling shared by tile, block and pixel coordinate calculations.
#[inline]
fn scale_by_power_of_two(value: i64, exponent: i16) -> i64 {
    if exponent >= 0 {
        value << exponent
    } else {
        value >> -exponent
    }
}

pub(super) fn render_tile_bitmap(
    journey_bitmap: &mut JourneyBitmap,
    view_x: i64,
    view_y: i64,
    zoom: i16,
    buffer_size_power: i16,
) -> BitMap2D {
    let mut bitmap = BitMap2D::new(buffer_size_power as u8);
    let zoom_diff = zoom - SOURCE_TILE_ZOOM;
    let tile_x = scale_by_power_of_two(view_x, -zoom_diff);
    let tile_y = scale_by_power_of_two(view_y, -zoom_diff);

    // Below source zoom, one view contains multiple source tiles. Above it,
    // one view selects a subdivision of a single source tile.
    let tile_span = 1 << (-zoom_diff).max(0);
    let zoom_factor = zoom_diff.max(0);
    let sub_tile_mask = (1 << zoom_factor) - 1;
    let sub_index = (view_x & sub_tile_mask, view_y & sub_tile_mask);
    let tile_width_power = zoom_diff + buffer_size_power;
    let size_power = tile_width_power.min(buffer_size_power);

    for i in 0..tile_span {
        for j in 0..tile_span {
            let tile_key = TileKey::new((tile_x + i) as u16, (tile_y + j) as u16);
            if !journey_bitmap.contains_tile(&tile_key) {
                continue;
            }
            add_tile_bits(
                &mut bitmap,
                journey_bitmap,
                &tile_key,
                RasterRegion {
                    origin: (
                        scale_by_power_of_two(i, tile_width_power),
                        scale_by_power_of_two(j, tile_width_power),
                    ),
                    sub_index,
                    zoom_factor,
                    size_power,
                },
            );
        }
    }
    bitmap
}

fn add_tile_bits(
    bitmap: &mut BitMap2D,
    journey_bitmap: &mut JourneyBitmap,
    tile_key: &TileKey,
    region: RasterRegion,
) {
    let RasterRegion {
        origin: (start_x, start_y),
        sub_index: (sub_tile_x, sub_tile_y),
        zoom_factor,
        size_power,
    } = region;
    let side = bitmap.side() as i64;
    debug_assert!(zoom_factor >= 0);
    debug_assert!(sub_tile_x < 1 << zoom_factor && sub_tile_y < 1 << zoom_factor);
    debug_assert!(journey_bitmap.contains_tile(tile_key));

    if size_power <= 0 {
        if start_x >= 0 && start_x < side && start_y >= 0 && start_y < side {
            bitmap.set(start_x as usize, start_y as usize, true);
        }
        return;
    }

    let block_num_power = TILE_WIDTH_OFFSET - zoom_factor;
    let block_start_x = scale_by_power_of_two(sub_tile_x, block_num_power);
    let block_start_y = scale_by_power_of_two(sub_tile_y, block_num_power);
    let block_span = 1_i64 << block_num_power.max(0);
    let block_zoom_factor = (-block_num_power).max(0);
    let sub_block_mask = (1 << block_zoom_factor) - 1;
    let sub_index = (sub_tile_x & sub_block_mask, sub_tile_y & sub_block_mask);
    let block_width_power = size_power - block_num_power;
    let block_size_power = block_width_power.min(i16::from(bitmap.width_exp()));

    if block_size_power <= 0 {
        let tile_summary = journey_bitmap.get_tile_summary(tile_key).unwrap();
        let block_end_x = block_start_x + block_span;
        let block_end_y = block_start_y + block_span;

        // Coarse rendering needs occupancy only. Walk occupied blocks without
        // deserializing the tile or probing every cell in a 128 × 128 grid.
        for block_key in tile_summary.iter_blocks() {
            let block_x = i64::from(block_key.x());
            let block_y = i64::from(block_key.y());
            if block_x < block_start_x
                || block_x >= block_end_x
                || block_y < block_start_y
                || block_y >= block_end_y
            {
                continue;
            }
            let x = start_x + scale_by_power_of_two(block_x - block_start_x, block_width_power);
            let y = start_y + scale_by_power_of_two(block_y - block_start_y, block_width_power);
            if x >= 0 && x < side && y >= 0 && y < side {
                bitmap.set(x as usize, y as usize, true);
            }
        }
        return;
    }

    let tile = journey_bitmap.get_tile(tile_key).unwrap();
    for i in 0..block_span {
        for j in 0..block_span {
            let block_key =
                BlockKey::from_x_y((block_start_x + i) as u8, (block_start_y + j) as u8);
            if let Some(block) = tile.get(&block_key) {
                add_block_bits(
                    bitmap,
                    block,
                    RasterRegion {
                        origin: (
                            start_x + scale_by_power_of_two(i, block_width_power),
                            start_y + scale_by_power_of_two(j, block_width_power),
                        ),
                        sub_index,
                        zoom_factor: block_zoom_factor,
                        size_power: block_size_power,
                    },
                );
            }
        }
    }
}

fn add_block_bits(bitmap: &mut BitMap2D, block: &Block, region: RasterRegion) {
    let RasterRegion {
        origin: (start_x, start_y),
        sub_index: (sub_block_x, sub_block_y),
        zoom_factor,
        size_power,
    } = region;
    debug_assert!(size_power > 0, "subpixel blocks are handled by the caller");

    // A whole block rendered below its native resolution can use its mipmap.
    if zoom_factor == 0 && size_power < BITMAP_WIDTH_OFFSET {
        for i in 0..(1 << size_power) {
            for j in 0..(1 << size_power) {
                if block.get_at_level(i as usize, j as usize, size_power as usize) {
                    bitmap.set((start_x + i) as usize, (start_y + j) as usize, true);
                }
            }
        }
        return;
    }

    let dot_num_power = BITMAP_WIDTH_OFFSET - zoom_factor;
    let dot_start_x = scale_by_power_of_two(sub_block_x, dot_num_power);
    let dot_start_y = scale_by_power_of_two(sub_block_y, dot_num_power);
    let dot_width_power = size_power - dot_num_power;
    let dot_width = 1 << dot_width_power.max(0);
    let dot_span = 1 << dot_num_power.max(0);

    for i in 0..dot_span {
        for j in 0..dot_span {
            let (dot_x, dot_y) = (dot_start_x + i, dot_start_y + j);
            if block.is_visited(dot_x as u8, dot_y as u8) {
                debug_assert!(dot_x < BITMAP_WIDTH && dot_y < BITMAP_WIDTH);
                set_rect_bits(
                    bitmap,
                    start_x + scale_by_power_of_two(i, dot_width_power),
                    start_y + scale_by_power_of_two(j, dot_width_power),
                    dot_width,
                    dot_width,
                );
            }
        }
    }
}

fn set_rect_bits(bitmap: &mut BitMap2D, x: i64, y: i64, w: i64, h: i64) {
    let side = bitmap.side() as i64;
    let x_end = (x + w).min(side);
    let y_end = (y + h).min(side);
    for py in y.max(0)..y_end {
        for px in x.max(0)..x_end {
            bitmap.set(px as usize, py as usize, true);
        }
    }
}
