use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

use flutter_rust_bridge::ZeroCopyBuffer;
use rusqlite::Transaction;
use simplelog::{Config, LevelFilter, WriteLogger};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Transform, Stroke};

use crate::storage::Storage;

struct MainState {
    storage: Storage,
}

static MAIN_STATE: OnceLock<MainState> = OnceLock::new();

pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) {
    let mut already_initialized = true;
    MAIN_STATE.get_or_init(|| {
        already_initialized = false;

        // init logging
        let path = Path::new(&cache_dir).join("main.log");
        WriteLogger::init(
            LevelFilter::Info,
            Config::default(),
            File::create(path).unwrap(),
        )
        .expect("Failed to initialize logging");

        let storage = Storage::init(temp_dir, doc_dir, support_dir, cache_dir);
        info!("initialized");
        MainState { storage }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

pub struct RenderResult {
    // coordinates are in lat or lng
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub data: ZeroCopyBuffer<Vec<u8>>,
}

// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn lng_lat_to_tile_xy(lng: f64, lat: f64, zoom: f32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom as i32);
    let lat_rad = (lat / 180.0) * std::f64::consts::PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * n;
    return (x.floor() as i32, y.floor() as i32);
}

pub fn tile_xy_to_lng_lat(x: i32, y: i32, zoom: f32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom as i32);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(
        std::f64::consts::PI * (1.0 - (2.0 * y as f64) / n),
    )) * 180.0)
        / std::f64::consts::PI;
    return (lng, lat);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

pub fn render_map_overlay(
    // map view area (coordinates are in lat or lng)
    zoom: f32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> RenderResult {
    // TODO: Change render backend. Right now we are using `tiny-skia`,
    // it should work just fine and we don't really need fancy features.
    // However, it is mostly a research project and does not feel like production ready,
    // `rust-skia` looks a lot better and has better performance (unlike `tiny-skia` is
    // purely on CPU, `rust-skia` can be ran on GPU). The reason we use `tiny-skia` right
    // now is that it is pure rust, so we don't need to think about how to build depenceies
    // for various platform.

    let (left_idx, top_idx) = lng_lat_to_tile_xy(left, top, zoom);
    let (right_idx, bottom_idx) = lng_lat_to_tile_xy(right, bottom, zoom);

    // TODO: reuse resources?
    const TILE_SIZE: u32 = 128;
    let width_by_tile: u32 = (right_idx - left_idx + 1).try_into().unwrap();
    let height_by_tile: u32 = (bottom_idx - top_idx + 1).try_into().unwrap();

    let mut pixmap = Pixmap::new(TILE_SIZE * width_by_tile, TILE_SIZE * height_by_tile).unwrap();
    pixmap.fill(Color::from_rgba8(0, 0, 0, 64));

    for x in 0..width_by_tile {
        for y in 0..height_by_tile {
            let mut pb = PathBuilder::new();
            pb.move_to((x*TILE_SIZE) as f32,( y*TILE_SIZE) as f32);
            pb.line_to(((x+1)*TILE_SIZE) as f32, (y*TILE_SIZE) as f32);
            pb.line_to(((x+1)*TILE_SIZE) as f32, ((y+1)*TILE_SIZE) as f32);
            pb.line_to(((x)*TILE_SIZE) as f32, ((y+1)*TILE_SIZE) as f32);
            pb.line_to((x*TILE_SIZE) as f32, (y*TILE_SIZE) as f32);
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

    let (overlay_left, overlay_top) = tile_xy_to_lng_lat(left_idx, top_idx, zoom);
    let (overlay_right, overlay_bottom) = tile_xy_to_lng_lat(right_idx + 1, bottom_idx + 1, zoom);

    RenderResult {
        top: overlay_top,
        left: overlay_left,
        right: overlay_right,
        bottom: overlay_bottom,
        data: ZeroCopyBuffer(bytes),
    }
}
