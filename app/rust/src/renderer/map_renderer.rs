use crate::api::api::CameraOption;
use crate::renderer::utils::image_to_png_data;
use crate::renderer::utils::{DEFAULT_BG_COLOR, DEFAULT_FG_COLOR, DEFAULT_TILE_SIZE};
use crate::renderer::TileRendererBasic;
use crate::renderer::TileRendererTrait;
use crate::{journey_bitmap::JourneyBitmap, utils};
use image::Rgba;
use image::RgbaImage;
use std::cmp::{max, min};

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
    journey_bitmap: JourneyBitmap,

    // For new web based renderer
    changed: bool,
    // TODO: when we switch to flutter side control, we should remove this
    provisioned_camera_option: Option<CameraOption>,

    // For old renderer
    tile_renderer: Box<dyn TileRendererTrait + Send + Sync>,
    bg_color: Rgba<u8>,
    fg_color: Rgba<u8>,
    current_render_area: Option<RenderArea>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        let tile_renderer = Box::new(TileRendererBasic::new(DEFAULT_TILE_SIZE));
        Self::new_with_tile_renderer(journey_bitmap, tile_renderer)
    }

    pub fn new_with_tile_renderer(
        journey_bitmap: JourneyBitmap,
        tile_renderer: Box<dyn TileRendererTrait + Send + Sync>,
    ) -> Self {
        Self {
            journey_bitmap,
            changed: false,
            provisioned_camera_option: None,
            tile_renderer,
            bg_color: DEFAULT_BG_COLOR,
            fg_color: DEFAULT_FG_COLOR,
            current_render_area: None,
        }
    }

    pub fn set_provisioned_camera_option(&mut self, camera_option: Option<CameraOption>) {
        self.provisioned_camera_option = camera_option;
    }

    pub fn get_provisioned_camera_option(&self) -> Option<CameraOption> {
        self.provisioned_camera_option
    }

    fn render_map_overlay(&self, render_area: &RenderArea) -> RenderResult {
        let tile_size: u32 = self.tile_renderer.get_tile_size().size();
        let width_by_tile: u32 = (render_area.right_idx - render_area.left_idx + 1)
            .try_into()
            .unwrap();
        let height_by_tile: u32 = (render_area.bottom_idx - render_area.top_idx + 1)
            .try_into()
            .unwrap();

        let mut image = RgbaImage::new(tile_size * width_by_tile, tile_size * height_by_tile);

        for x in 0..width_by_tile {
            for y in 0..height_by_tile {
                // TODO: cache?

                self.tile_renderer.render_on_image(
                    &mut image,
                    x * tile_size,
                    y * tile_size,
                    &self.journey_bitmap,
                    render_area.left_idx as i64 + x as i64,
                    render_area.top_idx as i64 + y as i64,
                    render_area.zoom as i16,
                    self.bg_color,
                    self.fg_color,
                );
            }
        }

        let (overlay_left, overlay_top) =
            utils::tile_x_y_to_lng_lat(render_area.left_idx, render_area.top_idx, render_area.zoom);
        let (overlay_right, overlay_bottom) = utils::tile_x_y_to_lng_lat(
            render_area.right_idx + 1,
            render_area.bottom_idx + 1,
            render_area.zoom,
        );

        let image_png = image_to_png_data(&image);

        RenderResult {
            width: tile_size * width_by_tile,
            height: tile_size * height_by_tile,
            top: overlay_top,
            left: overlay_left,
            right: overlay_right,
            bottom: overlay_bottom,
            data: image_png,
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
        self.changed = true;
        self.current_render_area = None;
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.changed = true;
        self.current_render_area = None;
    }

    pub fn reset(&mut self) {
        self.changed = true;
        self.current_render_area = None;
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn get_latest_bitmap_if_changed(&mut self) -> Option<&JourneyBitmap> {
        if self.changed {
            self.changed = false;
            Some(&self.journey_bitmap)
        } else {
            None
        }
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }
}
