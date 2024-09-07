use image;
use image::{ImageBuffer, Rgba};
use imageproc::filter::gaussian_blur_f32;
use std::cmp::{max, min};
use std::io::Cursor;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::{
    graphics::color_dilation2, journey_bitmap::JourneyBitmap, tile_renderer::TileRenderer, utils,
};

pub struct RenderResult {
    // coordinates are in lat or lng
    pub width: u32,
    pub height: u32,
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub data: Vec<u8>,
}

#[derive(PartialEq, Eq)]
struct RenderArea {
    zoom: i32,
    left_idx: i32,
    top_idx: i32,
    right_idx: i32,
    bottom_idx: i32,
}

pub struct MapRenderer {
    tile_renderer: TileRenderer,
    journey_bitmap: JourneyBitmap,
    current_render_area: Option<RenderArea>,
    dilation_radius: usize,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        let tile_renderer = TileRenderer::new();
        Self::new_with_tile_renderer(journey_bitmap, tile_renderer)
    }

    pub fn new_with_tile_renderer(
        journey_bitmap: JourneyBitmap,
        tile_renderer: TileRenderer,
    ) -> Self {
        Self {
            tile_renderer,
            journey_bitmap,
            current_render_area: None,
            dilation_radius: 1,
        }
    }

    pub fn set_dilation_radius(&mut self, radius: usize) {
        self.dilation_radius = radius;
    }

    fn render_map_overlay(&self, render_area: &RenderArea) -> RenderResult {
        // TODO: Change render backend. Right now we are using `tiny-skia`,
        // it should work just fine and we don't really need fancy features.
        // However, it is mostly a research project and does not feel like production ready,
        // `rust-skia` looks a lot better and has better performance (unlike `tiny-skia` is
        // purely on CPU, `rust-skia` can be ran on GPU). The reason we use `tiny-skia` right
        // now is that it is pure rust, so we don't need to think about how to build depenceies
        // for various platform.

        let tile_size: u32 = 1 << self.tile_renderer.get_tile_size_power();
        let width_by_tile: u32 = (render_area.right_idx - render_area.left_idx + 1)
            .try_into()
            .unwrap();
        let height_by_tile: u32 = (render_area.bottom_idx - render_area.top_idx + 1)
            .try_into()
            .unwrap();

        // TODO: reuse resurces?
        let mut pixmap =
            Pixmap::new(tile_size * width_by_tile, tile_size * height_by_tile).unwrap();
        // color must be set to the tile renderer directly upon its creation

        for x in 0..width_by_tile {
            for y in 0..height_by_tile {
                // TODO: cache?

                let tile_pixmap = self.tile_renderer.render_pixmap(
                    &self.journey_bitmap,
                    render_area.left_idx as u64 + x as u64,
                    render_area.top_idx as u64 + y as u64,
                    render_area.zoom as i16,
                );

                pixmap.draw_pixmap(
                    (x * tile_size) as i32,
                    (y * tile_size) as i32,
                    tile_pixmap.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }

        let width = pixmap.width();
        let height = pixmap.height();

        if self.dilation_radius > 0 {
            let color = self.tile_renderer.fg_color();
            color_dilation2(
                pixmap.pixels_mut(),
                width.try_into().unwrap(),
                height.try_into().unwrap(),
                color,
            );
        }

        // use imageproc to blur the pixmap, currently the imageproc library requires full ownership
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width, height, pixmap.data().to_vec())
                .expect("Error converting buffer to ImageBuffer");
        let blurred_image = gaussian_blur_f32(&img, 0.7);

        let mut png_buffer = Vec::new();
        let mut cursor = Cursor::new(&mut png_buffer);

        blurred_image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .unwrap();

        let (overlay_left, overlay_top) =
            utils::tile_x_y_to_lng_lat(render_area.left_idx, render_area.top_idx, render_area.zoom);
        let (overlay_right, overlay_bottom) = utils::tile_x_y_to_lng_lat(
            render_area.right_idx + 1,
            render_area.bottom_idx + 1,
            render_area.zoom,
        );

        RenderResult {
            width: pixmap.width(),
            height: pixmap.height(),
            top: overlay_top,
            left: overlay_left,
            right: overlay_right,
            bottom: overlay_bottom,
            data: png_buffer,
        }
    }

    pub fn maybe_render_map_overlay(
        &mut self,
        // map view area (coordinates are in lat or lng)
        zoom: i32,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    ) -> Option<RenderResult> {
        // TODO: This doesn't really work when antimeridian is involved, see
        // the upstream issue: https://github.com/maplibre/maplibre-native/issues/1681
        let (mut left_idx, mut top_idx) = utils::lng_lat_to_tile_x_y(left, top, zoom);
        let (mut right_idx, mut bottom_idx) = utils::lng_lat_to_tile_x_y(right, bottom, zoom);

        // TODO: There is a hack to make sure we always cover a bit bigger to
        // avoid the gap between user move to new area and drawing that area.
        let n = f64::powi(2.0, zoom) as i32;
        top_idx = max(top_idx - 1, 0);
        bottom_idx = min(bottom_idx + 1, n - 1);
        left_idx -= 1;
        right_idx += 1;
        if (right_idx - left_idx).abs() >= n {
            left_idx = 0;
            right_idx = n - 1;
        } else {
            if left_idx < 0 {
                left_idx += n;
            }
            while right_idx < left_idx {
                right_idx += n;
            }
        }

        let render_area = RenderArea {
            zoom,
            left_idx,
            top_idx,
            right_idx,
            bottom_idx,
        };
        if Some(&render_area) == self.current_render_area.as_ref() {
            // same, nothing to do
            None
        } else {
            let render_result = self.render_map_overlay(&render_area);
            self.current_render_area = Some(render_area);
            Some(render_result)
        }
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: Fn(&mut JourneyBitmap),
    {
        f(&mut self.journey_bitmap);
        // TODO: we should improve the cache invalidation rule
        self.current_render_area = None;
    }

    pub fn reset(&mut self) {
        self.current_render_area = None;
    }
}
