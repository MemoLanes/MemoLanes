// cherry picked from https://github.com/tavimori/fogcore/blob/c965ca5bff830924520fb156171e6bedefd39e5d/src/renderer.rs
// TODO: clean up the code

use crate::journey_bitmap::{Block, JourneyBitmap, Tile};
use crate::journey_bitmap::{BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, TILE_WIDTH_OFFSET};
use tiny_skia;
use tiny_skia::PremultipliedColorU8;

const TILE_ZOOM: i16 = 9;
pub const DEFAULT_VIEW_SIZE_POWER: i16 = 9; // default view size is 2^8 = 256

// we have 512*512 tiles, 128*128 blocks and a single block contains a 64*64 bitmap.
pub struct TileRenderer {
    view_size_power: i16,
    bg_color_prgba: PremultipliedColorU8,
    fg_color_prgba: PremultipliedColorU8,
}

impl TileRenderer {
    pub fn new() -> Self {
        let opacity = 0.5;
        let alpha = (opacity * 255.0) as u8;
        let bg_color_prgba = PremultipliedColorU8::from_rgba(0, 0, 0, alpha).unwrap();
        let fg_color_prgba = PremultipliedColorU8::TRANSPARENT;
        Self {
            view_size_power: DEFAULT_VIEW_SIZE_POWER,
            bg_color_prgba,
            fg_color_prgba,
        }
    }

    pub fn new_with_color(fg_color: PremultipliedColorU8, bg_color: PremultipliedColorU8) -> Self {
        let fg_color_prgba = fg_color;
        let bg_color_prgba = bg_color;
        Self {
            view_size_power: DEFAULT_VIEW_SIZE_POWER,
            bg_color_prgba,
            fg_color_prgba,
        }
    }

    pub fn fg_color(&self) -> PremultipliedColorU8 {
        self.fg_color_prgba
    }

    pub fn bg_color(&self) -> PremultipliedColorU8 {
        self.bg_color_prgba
    }

    pub fn set_tile_size_power(&mut self, power: i16) {
        self.view_size_power = power;
    }

    pub fn get_tile_size_power(&self) -> i16 {
        self.view_size_power
    }

    /// Render a given location of FogMap data onto a Pixmap.
    ///
    /// * `journey_bitmap`: an instance of JourneyBitmap.
    /// * `tile_x`: x-index of a tile, provided the zoom level.
    /// * `tile_y`: y-index of a tile, provided the zoom level.
    /// * `zoom`: zoom levels. Please refer to [OSM zoom levels](https://wiki.openstreetmap.org/wiki/Zoom_levels) for more infomation.
    /// * `width`: width of an image in pixels.
    // TODO: may use mipmap to accelerate the rendering.
    // TODO: currently if a pixel contains multiple tile / block, the rendering process will write over the pixel multiple times, may use other interpolation method.
    // We use a method called max-pooling interpolation to enlarge the tracks while keeping them easy to see at different sizes.
    pub fn render_pixmap(
        &self,
        journey_bitmap: &JourneyBitmap,
        view_x: u64,
        view_y: u64,
        zoom: i16,
    ) -> tiny_skia::Pixmap {
        let width = 1 << self.view_size_power;
        let mut pixmap = tiny_skia::Pixmap::new(width, width).unwrap();
        let pixels = pixmap.pixels_mut();

        // draw background
        for p in pixels.iter_mut() {
            *p = self.bg_color_prgba;
        }

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

                    let tile_width_power = zoom_diff_view_to_tile + self.view_size_power;

                    // tile shift for the (i,j)th tile in this view
                    let (x0, y0) = if tile_width_power > 0 {
                        (i << tile_width_power, j << tile_width_power)
                    } else {
                        (i >> -tile_width_power, j >> -tile_width_power)
                    };
                    self.render_tile_on_pixels(
                        tile,
                        pixels,
                        x0,
                        y0,
                        sub_tile_x_idx,
                        sub_tile_y_idx,
                        zoom_factor,
                        std::cmp::min(tile_width_power, self.view_size_power),
                    );
                }
            }
        }
        pixmap
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tile_on_pixels(
        &self,
        tile: &Tile,
        pixels: &mut [tiny_skia::PremultipliedColorU8],
        start_x: u64,
        start_y: u64,
        sub_tile_x_idx: u64,
        sub_tile_y_idx: u64,
        zoom_factor: i16,
        size_power: i16,
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
            self.draw_pixel(pixels, start_x, start_y);
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
                        self.render_block_on_pixels(
                            block,
                            pixels,
                            start_x + offset_x,
                            start_y + offset_y,
                            sub_block_x_idx,
                            sub_block_y_idx,
                            block_zoom_factor,
                            std::cmp::min(block_width_power, self.view_size_power),
                        );
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_block_on_pixels(
        &self,
        block: &Block,
        pixels: &mut [tiny_skia::PremultipliedColorU8],
        start_x: u64,
        start_y: u64,
        sub_block_x_idx: u64,
        sub_block_y_idx: u64,
        zoom_factor: i16,
        size_power: i16,
    ) {
        if size_power <= 0 {
            self.draw_pixel(pixels, start_x, start_y);
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
                        debug_assert!(dot_x < BITMAP_WIDTH as u64);
                        debug_assert!(dot_y < BITMAP_WIDTH as u64);
                        let (offset_x, offset_y) = if block_dot_width_power >= 0 {
                            (i << block_dot_width_power, j << block_dot_width_power)
                        } else {
                            (i >> -block_dot_width_power, j >> -block_dot_width_power)
                        };
                        self.draw_rect(
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

    fn draw_pixel(&self, pixels: &mut [tiny_skia::PremultipliedColorU8], x: u64, y: u64) {
        // according to tiny-skia docs, the pixel data is not aligned, therefore pixels can be accessed dirrecly by `pixels[x*width + y]`
        let index = x + (y << self.view_size_power);
        pixels[index as usize] = self.fg_color_prgba;
    }

    fn draw_rect(
        &self,
        pixels: &mut [tiny_skia::PremultipliedColorU8],
        x: u64,
        y: u64,
        w: u64,
        h: u64,
    ) {
        for i in x..(x + w) {
            for j in y..(y + h) {
                self.draw_pixel(pixels, i, j);
            }
        }
    }
}
