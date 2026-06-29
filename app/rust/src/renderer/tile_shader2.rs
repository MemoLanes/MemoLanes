use journey_kernel::bitmap2d::BitMap2D;

use crate::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey};
use crate::journey_bitmap::{BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, TILE_WIDTH_OFFSET};

const TILE_ZOOM: i16 = 9;

// we have 512*512 tiles, 128*128 blocks and a single block contains a 64*64 bitmap.
pub struct TileShader2;

impl TileShader2 {
    pub fn render_tile_bitmap(
        journey_bitmap: &mut JourneyBitmap,
        view_x: i64,
        view_y: i64,
        zoom: i16,
        buffer_size_power: i16,
    ) -> BitMap2D {
        let mut bitmap = BitMap2D::new(buffer_size_power as u8);
        let side = bitmap.side() as i64;

        // https://developers.google.com/maps/documentation/javascript/coordinates
        let zoom_diff_view_to_tile = zoom - TILE_ZOOM;

        // when view has larger zoom, the view_x is larger than tile_x (but the region of view is smaller)
        let (tile_x, tile_y) = if zoom_diff_view_to_tile > 0 {
            (
                view_x >> zoom_diff_view_to_tile,
                view_y >> zoom_diff_view_to_tile,
            )
        } else {
            (
                view_x << -zoom_diff_view_to_tile,
                view_y << -zoom_diff_view_to_tile,
            )
        };

        // when zoom_diff_view_to_tile < 0, a view contains multiple tiles.
        for i in 0..(1 << std::cmp::max(-zoom_diff_view_to_tile, 0)) {
            for j in 0..(1 << std::cmp::max(-zoom_diff_view_to_tile, 0)) {
                let tile_key = TileKey::new((tile_x + i) as u16, (tile_y + j) as u16);

                if journey_bitmap.contains_tile(&tile_key) {
                    let zoom_factor = std::cmp::max(0, zoom_diff_view_to_tile);
                    let (sub_tile_x_idx, sub_tile_y_idx) = if zoom_factor > 0 {
                        let mask = (1 << zoom_factor) - 1;
                        ((view_x) & mask, (view_y) & mask)
                    } else {
                        (0, 0)
                    };

                    let tile_width_power = zoom_diff_view_to_tile + buffer_size_power;

                    // tile shift for the (i,j)th tile in this view
                    let (x0, y0) = if tile_width_power > 0 {
                        (i << tile_width_power, j << tile_width_power)
                    } else {
                        (i >> -tile_width_power, j >> -tile_width_power)
                    };
                    Self::add_tile_bits(
                        &mut bitmap,
                        side,
                        journey_bitmap,
                        &tile_key,
                        x0,
                        y0,
                        sub_tile_x_idx,
                        sub_tile_y_idx,
                        zoom_factor,
                        std::cmp::min(tile_width_power, buffer_size_power),
                        buffer_size_power,
                    );
                }
            }
        }

        bitmap
    }

    #[allow(clippy::too_many_arguments)]
    fn add_tile_bits(
        bitmap: &mut BitMap2D,
        side: i64,
        journey_bitmap: &mut JourneyBitmap,
        tile_key: &TileKey,
        start_x: i64,
        start_y: i64,
        sub_tile_x_idx: i64,
        sub_tile_y_idx: i64,
        zoom_factor: i16,
        size_power: i16,
        buffer_size_power: i16,
    ) {
        debug_assert!(
            zoom_factor >= 0,
            "tile zoom factor must be greater or equal to zero"
        );
        debug_assert!(
            sub_tile_x_idx <= 1 << zoom_factor,
            "sub_tile_x_idx cannot exceed the tile"
        );
        debug_assert!(
            sub_tile_y_idx <= 1 << zoom_factor,
            "sub_tile_y_idx cannot exceed the tile"
        );
        debug_assert!(
            journey_bitmap.contains_tile(tile_key),
            "tile must exist in bitmap"
        );

        if size_power <= 0 {
            if start_x >= 0 && start_x < side && start_y >= 0 && start_y < side {
                bitmap.set(start_x as usize, start_y as usize, true);
            }
        } else {
            let block_num_power = TILE_WIDTH_OFFSET - zoom_factor;
            let (block_start_x, block_start_y) = if block_num_power >= 0 {
                (
                    sub_tile_x_idx << block_num_power,
                    sub_tile_y_idx << block_num_power,
                )
            } else {
                (
                    sub_tile_x_idx >> -block_num_power,
                    sub_tile_y_idx >> -block_num_power,
                )
            };

            let block_zoom_factor = std::cmp::max(0, -block_num_power);
            let (sub_block_x_idx, sub_block_y_idx) = if block_zoom_factor > 0 {
                let mask = (1 << block_zoom_factor) - 1;
                ((sub_tile_x_idx) & mask, (sub_tile_y_idx) & mask)
            } else {
                (0, 0)
            };
            let block_width_power = size_power - block_num_power;
            let block_size_power = std::cmp::min(block_width_power, buffer_size_power);

            let tile_info = if block_size_power <= 0 {
                either::Left(journey_bitmap.get_tile_summary(tile_key).unwrap())
            } else {
                either::Right(journey_bitmap.get_tile(tile_key).unwrap())
            };

            for i in 0..(1 << std::cmp::max(block_num_power, 0)) {
                for j in 0..(1 << std::cmp::max(block_num_power, 0)) {
                    let block_key =
                        BlockKey::from_x_y((block_start_x + i) as u8, (block_start_y + j) as u8);
                    let (offset_x, offset_y) = if block_width_power >= 0 {
                        (i << block_width_power, j << block_width_power)
                    } else {
                        (i >> -block_width_power, j >> -block_width_power)
                    };
                    let block_start_x = start_x + offset_x;
                    let block_start_y = start_y + offset_y;
                    match &tile_info {
                        either::Left(tile_summary) => {
                            if tile_summary.contains_block(&block_key)
                                && block_start_x >= 0
                                && block_start_x < side
                                && block_start_y >= 0
                                && block_start_y < side
                            {
                                bitmap.set(block_start_x as usize, block_start_y as usize, true);
                            }
                        }
                        either::Right(tile) => {
                            if let Some(block) = tile.get(&block_key) {
                                Self::add_block_bits(
                                    bitmap,
                                    side,
                                    block,
                                    block_start_x,
                                    block_start_y,
                                    sub_block_x_idx,
                                    sub_block_y_idx,
                                    block_zoom_factor,
                                    block_size_power,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_block_bits(
        bitmap: &mut BitMap2D,
        side: i64,
        block: &Block,
        start_x: i64,
        start_y: i64,
        sub_block_x_idx: i64,
        sub_block_y_idx: i64,
        zoom_factor: i16,
        size_power: i16,
    ) {
        debug_assert!(
            size_power > 0,
            "`size_power <= 0` should already be handled in the caller function"
        );

        let dot_num_power = BITMAP_WIDTH_OFFSET - zoom_factor;

        let (dot_start_x, dot_start_y) = if dot_num_power >= 0 {
            (
                sub_block_x_idx << dot_num_power,
                sub_block_y_idx << dot_num_power,
            )
        } else {
            (
                sub_block_x_idx >> -dot_num_power,
                sub_block_y_idx >> -dot_num_power,
            )
        };

        let block_dot_width_power = size_power - (BITMAP_WIDTH_OFFSET - zoom_factor);
        let block_dot_width = 1 << std::cmp::max(0, block_dot_width_power);

        for i in 0..(1 << std::cmp::max(dot_num_power, 0)) {
            for j in 0..(1 << std::cmp::max(dot_num_power, 0)) {
                let (dot_x, dot_y) = (dot_start_x + i, dot_start_y + j);
                if block.is_visited(dot_x as u8, dot_y as u8) {
                    debug_assert!(dot_x < BITMAP_WIDTH);
                    debug_assert!(dot_y < BITMAP_WIDTH);
                    let (offset_x, offset_y) = if block_dot_width_power >= 0 {
                        (i << block_dot_width_power, j << block_dot_width_power)
                    } else {
                        (i >> -block_dot_width_power, j >> -block_dot_width_power)
                    };
                    Self::set_rect_bits(
                        bitmap,
                        side,
                        start_x + offset_x,
                        start_y + offset_y,
                        block_dot_width,
                        block_dot_width,
                    );
                }
            }
        }
    }

    fn set_rect_bits(bitmap: &mut BitMap2D, side: i64, x: i64, y: i64, w: i64, h: i64) {
        let x_end = std::cmp::min(x + w, side);
        let y_end = std::cmp::min(y + h, side);
        let x_start = std::cmp::max(x, 0);
        let y_start = std::cmp::max(y, 0);
        for py in y_start..y_end {
            for px in x_start..x_end {
                bitmap.set(px as usize, py as usize, true);
            }
        }
    }
}
