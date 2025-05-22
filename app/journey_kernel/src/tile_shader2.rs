use crate::journey_bitmap::{Block, JourneyBitmap, Tile};
use crate::journey_bitmap::{BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, TILE_WIDTH_OFFSET};

const TILE_ZOOM: i16 = 9;

// we have 512*512 tiles, 128*128 blocks and a single block contains a 64*64 bitmap.
pub struct TileShader2;

impl TileShader2 {
    pub fn get_pixels_coordinates(
        start_x: u32,
        start_y: u32,
        journey_bitmap: &JourneyBitmap,
        view_x: i64,
        view_y: i64,
        zoom: i16,
        buffer_size_power: i16,
    ) -> Vec<(i64, i64)> {
        let mut pixels = Vec::new();

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
                // draw tile tile_x+i, tile_y+j

                if let Some(tile) = journey_bitmap
                    .tiles
                    .get(&((tile_x + i) as u16, (tile_y + j) as u16))
                {
                    // if zoom_diff_view_to_tile > 0, view zoom larger, view region smaller, draw a portion of a single tile.
                    // if zoom_diff_view_to_tile < 0, view zoom smaller, view region larger, draw the full tile at given location of view.

                    // tile_width in pixels
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
                    Self::add_tile_pixels(
                        &mut pixels,
                        tile,
                        start_x as i64 + x0,
                        start_y as i64 + y0,
                        sub_tile_x_idx,
                        sub_tile_y_idx,
                        zoom_factor,
                        std::cmp::min(tile_width_power, buffer_size_power),
                        buffer_size_power,
                    );
                }
            }
        }

        pixels
    }

    fn add_tile_pixels(
        pixels: &mut Vec<(i64, i64)>,
        tile: &Tile,
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

        if size_power <= 0 {
            // the tile only occupies at most one pixel, so we don't have to access the blocks.
            // sub_image.put_pixel(start_x as u32, start_y as u32, fg_color);
            pixels.push((start_x, start_y));
        } else {
            // the tile occupies more than one pixel, currently all the blocks will be used to render。

            let block_num_power = TILE_WIDTH_OFFSET - zoom_factor; // number of block in a row of the view
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

            for i in 0..(1 << std::cmp::max(block_num_power, 0)) {
                for j in 0..(1 << std::cmp::max(block_num_power, 0)) {
                    if let Some(block) = tile
                        .blocks
                        .get(&((block_start_x + i) as u8, (block_start_y + j) as u8))
                    {
                        let (offset_x, offset_y) = if block_width_power >= 0 {
                            (i << block_width_power, j << block_width_power)
                        } else {
                            (i >> -block_width_power, j >> -block_width_power)
                        };
                        Self::add_block_pixels(
                            pixels,
                            block,
                            start_x + offset_x,
                            start_y + offset_y,
                            sub_block_x_idx,
                            sub_block_y_idx,
                            block_zoom_factor,
                            std::cmp::min(block_width_power, buffer_size_power),
                        );
                    }
                }
            }
        }
    }

    fn add_block_pixels(
        pixels: &mut Vec<(i64, i64)>,
        block: &Block,
        start_x: i64,
        start_y: i64,
        sub_block_x_idx: i64,
        sub_block_y_idx: i64,
        zoom_factor: i16,
        size_power: i16,
    ) {
        if size_power <= 0 {
            pixels.push((start_x, start_y));
        } else {
            let dot_num_power = BITMAP_WIDTH_OFFSET - zoom_factor; // number of block in a row of the view

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
                        Self::add_rect_pixels(
                            pixels,
                            start_x + offset_x,
                            start_y + offset_y,
                            block_dot_width,
                            block_dot_width,
                        );
                    }
                }
            }
        }
    }

    fn add_rect_pixels(pixels: &mut Vec<(i64, i64)>, x: i64, y: i64, w: i64, h: i64) {
        for i in x..(x + w) {
            for j in y..(y + h) {
                pixels.push((i, j));
            }
        }
    }
}
