use flutter_rust_bridge::ZeroCopyBuffer;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub struct RenderResult {
    // coordinates are in lat or lng
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub data: ZeroCopyBuffer<Vec<u8>>,
}

// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
fn lng_lat_to_tile_xy(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom);
    let lat_rad = (lat / 180.0) * std::f64::consts::PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

fn tile_xy_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(
        std::f64::consts::PI * (1.0 - (2.0 * y as f64) / n),
    )) * 180.0)
        / std::f64::consts::PI;
    (lng, lat)
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
    current_render_area: Option<RenderArea>,
}

impl MapRenderer {
    pub fn new() -> Self {
        MapRenderer {
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

        const TILE_SIZE: u32 = 128;
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
                // TODO: draw idx on the tile. Sadly, `tiny-skia` does not support this, we could use
                // https://docs.rs/text-to-png/latest/text_to_png/ This is not so efficient but should
                // be good enough for debugging.
                let mut pb = PathBuilder::new();
                pb.move_to((x * TILE_SIZE) as f32, (y * TILE_SIZE) as f32);
                pb.line_to(((x + 1) * TILE_SIZE) as f32, (y * TILE_SIZE) as f32);
                pb.line_to(((x + 1) * TILE_SIZE) as f32, ((y + 1) * TILE_SIZE) as f32);
                pb.line_to(((x) * TILE_SIZE) as f32, ((y + 1) * TILE_SIZE) as f32);
                pb.line_to((x * TILE_SIZE) as f32, (y * TILE_SIZE) as f32);
                pb.close();

                let path = pb.finish().unwrap();

                let mut paint = Paint::default();
                paint.set_color_rgba8(0, 0, 0, 128);
                pixmap.stroke_path(
                    &path,
                    &paint,
                    &Stroke::default(),
                    Transform::identity(),
                    None,
                );
            }
        }

        let bytes = pixmap.encode_png().unwrap();

        let (overlay_left, overlay_top) =
            tile_xy_to_lng_lat(render_area.left_idx, render_area.top_idx, render_area.zoom);
        let (overlay_right, overlay_bottom) = tile_xy_to_lng_lat(
            render_area.right_idx + 1,
            render_area.bottom_idx + 1,
            render_area.zoom,
        );

        RenderResult {
            top: overlay_top,
            left: overlay_left,
            right: overlay_right,
            bottom: overlay_bottom,
            data: ZeroCopyBuffer(bytes),
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
        let (left_idx, top_idx) = lng_lat_to_tile_xy(left, top, zoom);
        let (mut right_idx, bottom_idx) = lng_lat_to_tile_xy(right, bottom, zoom);

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
}
