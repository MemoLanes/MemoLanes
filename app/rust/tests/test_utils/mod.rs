use chrono::NaiveDate;
use memolanes_core::cache_db::{self, CacheDb};
use memolanes_core::geo::GeoIndex;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::JourneyKind;
use memolanes_core::main_db::MainDb;
use memolanes_core::main_db::NewJourney;
use memolanes_core::renderer::map_renderer::*;
use memolanes_core::utils;
mod render_utils;
use render_utils::*;
use tempdir::TempDir;

use image::RgbaImage;
use sha2::{Digest, Sha256};
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

pub const START_LNG: f64 = 151.1435370795134;
pub const START_LAT: f64 = -33.793291910360125;
pub const END_LNG: f64 = 151.2783692841415;
pub const END_LAT: f64 = -33.943600147192235;
pub const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
pub const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

#[derive(PartialEq, Eq)]
pub struct RenderArea {
    pub zoom: i32,
    pub left_idx: i32,
    pub top_idx: i32,
    pub right_idx: i32,
    pub bottom_idx: i32,
}

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

/// Open a synthetic geo asset from its bytes; `dir` must outlive the index.
pub fn geo_index_from_bytes(dir: &TempDir, bytes: &[u8]) -> GeoIndex {
    let path = dir.path().join("geo_data.bin");
    fs::write(&path, bytes).unwrap();
    GeoIndex::open(&path).unwrap()
}

static IMAGE_HASHES: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../image_hashes.lock")).unwrap());
static GPX_HASHES: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../gpx_hashes.lock")).unwrap());

pub fn verify_image(name: &str, image: &[u8]) {
    check_image(name, image).unwrap_or_else(|error| panic!("{error}"));
}

/// Return a mismatch instead of panicking, allowing a suite to inspect every case.
pub fn check_image(name: &str, image: &[u8]) -> Result<(), String> {
    check_snapshot(name, "png", image, &IMAGE_HASHES, "image_hashes.lock")
}

pub fn check_gpx(name: &str, gpx: &[u8]) -> Result<(), String> {
    check_snapshot(name, "gpx", gpx, &GPX_HASHES, "gpx_hashes.lock")
}

/// Save failed snapshots before reporting the mismatch. Set GENERATE_INSPECTION_FILES=1
/// to also save passing snapshots. Baselines are never created or updated by tests.
fn check_snapshot(
    name: &str,
    extension: &str,
    bytes: &[u8],
    hashes: &BTreeMap<String, String>,
    baseline: &str,
) -> Result<(), String> {
    let actual = hex::encode(Sha256::digest(bytes));
    let expected = hashes.get(name);
    let matches = expected == Some(&actual);
    let export = std::env::var("GENERATE_INSPECTION_FILES").is_ok_and(|value| value == "1");
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/for_inspection")
        .join(format!("{name}.{extension}"));
    if !matches || export {
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, bytes).unwrap();
    }
    if matches {
        Ok(())
    } else {
        Err(format!(
            "Snapshot {name}.{extension} changed (expected {expected:?}). Inspect {} before \
             updating tests/{baseline}. Actual entry: \"{name}\": \"{actual}\"",
            output.display(),
        ))
    }
}

pub fn render_map_overlay(
    map_renderer: &mut MapRenderer,
    // map view area (coordinates are in lat or lng)
    zoom: i32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> RenderResult {
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

    render_map_overlay_internal(map_renderer, &render_area)
}

fn render_map_overlay_internal(
    map_renderer: &mut MapRenderer,
    render_area: &RenderArea,
) -> RenderResult {
    /* for test, map_renderer initialized by MapRenderer::new, tilerenderer size is default size.  */
    let tile_size: u32 = DEFAULT_TILE_SIZE.size(); //512
    let width_by_tile: u32 = (render_area.right_idx - render_area.left_idx + 1)
        .try_into()
        .unwrap();
    let height_by_tile: u32 = (render_area.bottom_idx - render_area.top_idx + 1)
        .try_into()
        .unwrap();

    let mut image = RgbaImage::new(tile_size * width_by_tile, tile_size * height_by_tile);
    // Rendering lazily populates bitmap caches, so use a private test copy
    // instead of borrowing `MapRenderer::update` for a non-update operation.
    let mut journey_bitmap = map_renderer.peek_latest_bitmap().clone();

    for x in 0..width_by_tile {
        for y in 0..height_by_tile {
            TileShader::render_on_image(
                &mut image,
                x * tile_size,
                y * tile_size,
                &mut journey_bitmap,
                render_area.left_idx as i64 + x as i64,
                render_area.top_idx as i64 + y as i64,
                render_area.zoom as i16,
                DEFAULT_TILE_SIZE.power(), // 9
                DEFAULT_BG_COLOR,
                DEFAULT_FG_COLOR,
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

pub fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
pub fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}
pub fn draw_line3(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(MID_LNG, START_LAT, MID_LNG, END_LAT)
}
pub fn draw_line4(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, MID_LAT, END_LNG, MID_LAT)
}

pub fn draw_sample_bitmap() -> JourneyBitmap {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    journey_bitmap
}

/// Whichever cache backend this tree ships, via `cache_db::new` — so callers
/// that only assert on what a `CacheDb` answers stay valid regardless of the
/// backend. Tests that need a concrete backend's storage internals build it
/// themselves.
pub fn setup_main_and_cache_db(prefix: &str) -> (MainDb, impl CacheDb, TempDir, TempDir) {
    let main_dir = TempDir::new(&format!("{prefix}-main")).unwrap();
    let cache_dir = TempDir::new(&format!("{prefix}-cache")).unwrap();
    let main_db = MainDb::open(main_dir.path().to_str().unwrap()).unwrap();
    let cache_db = cache_db::new(cache_dir.path().to_str().unwrap());
    (main_db, cache_db, main_dir, cache_dir)
}

pub fn insert_bitmap_journey(
    txn: &mut memolanes_core::main_db::Txn,
    date: NaiveDate,
    kind: JourneyKind,
    bitmap: JourneyBitmap,
) -> String {
    txn.create_and_insert_journey(NewJourney {
        journey_date: date,
        start: None,
        end: None,
        created_at: None,
        journey_kind: kind,
        note: None,
        journey_data: JourneyData::Bitmap(bitmap),
        raw_data: None,
    })
    .unwrap()
}

pub fn make_bitmap_with_line(line_fn: fn(&mut JourneyBitmap)) -> JourneyBitmap {
    let mut bitmap = JourneyBitmap::new();
    line_fn(&mut bitmap);
    bitmap
}

pub fn geo_asset(worldview_id: &str) -> std::path::PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/geo")
        .join(format!("geo_data_{worldview_id}.bin"));
    assert!(
        path.exists(),
        "{} is absent — run `just rasterize-geo` (it is a gitignored build product). \
         This test asserts against the REAL shipped asset and must not silently pass without it.",
        path.display()
    );
    path
}

/// Supply stable metadata while exercising the same writer as journey exports.
pub fn vector_export(
    vector: memolanes_core::journey_vector::JourneyVector,
) -> memolanes_core::export_data::JourneyExport {
    memolanes_core::export_data::JourneyExport {
        journey_date: NaiveDate::from_ymd_opt(2026, 9, 11).unwrap(),
        vector,
    }
}
