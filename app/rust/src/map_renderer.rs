use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

use crate::{
    journey_bitmap::JourneyBitmap,
    tile_renderer::{self, TileRenderer},
    utils,
};

pub struct RenderResult {
    // coordinates are in lat or lng
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
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        MapRenderer {
            tile_renderer: TileRenderer::new(),
            journey_bitmap,
            current_render_area: None,
        }
    }

    fn render_map_overlay(&self, render_area: &RenderArea) -> RenderResult {
        // TODO: Change render backend. Right now we are using `tiny-skia`,
        // it should work just fine and we don't really need fancy features.
        // However, it is mostly a research project and does not feel like production ready,
        // `rust-skia` looks a lot better and has better performance (unlike `tiny-skia` is
        // purely on CPU, `rust-skia` can be ran on GPU). The reason we use `tiny-skia` right
        // now is that it is pure rust, so we don't need to think about how to build depenceies
        // for various platform.

        const TILE_SIZE: u32 = 1 << tile_renderer::DEFAULT_VIEW_SIZE_POWER;
        let width_by_tile: u32 = (render_area.right_idx - render_area.left_idx + 1)
            .try_into()
            .unwrap();
        let height_by_tile: u32 = (render_area.bottom_idx - render_area.top_idx + 1)
            .try_into()
            .unwrap();

        // TODO: reuse resurces?
        let mut pixmap =
            Pixmap::new(TILE_SIZE * width_by_tile, TILE_SIZE * height_by_tile).unwrap();
        pixmap.fill(Color::from_rgba8(0, 0, 0, 64));

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
                    (x * TILE_SIZE) as i32,
                    (y * TILE_SIZE) as i32,
                    tile_pixmap.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }

        let bytes = pixmap.encode_png().unwrap();

        let (overlay_left, overlay_top) =
            utils::tile_x_y_to_lng_lat(render_area.left_idx, render_area.top_idx, render_area.zoom);
        let (overlay_right, overlay_bottom) = utils::tile_x_y_to_lng_lat(
            render_area.right_idx + 1,
            render_area.bottom_idx + 1,
            render_area.zoom,
        );

        RenderResult {
            top: overlay_top,
            left: overlay_left,
            right: overlay_right,
            bottom: overlay_bottom,
            data: bytes,
        }
    }

    pub fn maybe_render_map_overlay(
        &mut self,
        // map view area (coordinates are in lat or lng)
        zoom: f32,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
    ) -> Option<RenderResult> {
        // TODO: This doesn't really work when antimeridian is involved, see
        // the upstream issue: https://github.com/maplibre/maplibre-native/issues/1681
        let zoom = zoom as i32;
        let (left_idx, top_idx) = utils::lng_lat_to_tile_x_y(left, top, zoom);
        let (mut right_idx, bottom_idx) = utils::lng_lat_to_tile_x_y(right, bottom, zoom);

        if right_idx < left_idx {
            let n = f64::powi(2.0, zoom) as i32;
            right_idx += n;
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
}
