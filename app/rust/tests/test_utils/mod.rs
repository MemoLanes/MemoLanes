use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::renderer::map_renderer::*;
mod render_utils;
use render_utils::*;

use image::RgbaImage;
use serde_json;
use sha2::{Digest, Sha256};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::path::Path;
use std::{fs::File, io::Write};

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;
const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

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

pub fn verify_image(name: &str, image: &Vec<u8>) {
    let hash_table_path = "tests/image_hashes.lock";
    let mut hash_table: HashMap<String, String> = if Path::new(hash_table_path).exists() {
        let hash_table_content =
            fs::read_to_string(hash_table_path).expect("Failed to read hash table file");
        serde_json::from_str(&hash_table_content).unwrap_or_else(|_| HashMap::new())
    } else {
        HashMap::new()
    };

    // Calculate hash of the current image
    let mut hasher = Sha256::new();
    hasher.update(image);
    let current_hash = format!("{:x}", hasher.finalize());

    if let Some(stored_hash) = hash_table.get(name) {
        // Entry exists, compare hashes
        assert_eq!(
            &current_hash, stored_hash,
            "Image hash mismatch for {}. Expected: {}, Got: {}. If you have updated the image, please delete the image_hashes.lock file and re-run the tests.",
            name, stored_hash, current_hash
        );
        println!("Verified image hash for: {}", name);
    } else {
        // No entry exists, add new entry
        hash_table.insert(name.to_string(), current_hash.clone());
        let hash_table_content =
            serde_json::to_string_pretty(&hash_table).expect("Failed to serialize hash table");
        fs::write(hash_table_path, hash_table_content).expect("Failed to write hash table file");
        println!("Added new hash entry for: {}", name);
    }

    // Always save the image file
    let output_path = format!("tests/for_inspection/{}.png", name);
    let mut file = File::create(&output_path).expect("Failed to create file");
    file.write_all(image).expect("Failed to write to file");
    println!("Saved image file: {}", output_path);
}

pub fn lng_lat_to_tile_x_y(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom);
    let lat_rad = (lat / 180.0) * PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

pub fn tile_x_y_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(PI * (1.0 - (2.0 * y as f64) / n))) * 180.0) / PI;
    (lng, lat)
}

pub fn maybe_render_map_overlay(
    map_renderer: &mut MapRenderer,
    // map view area (coordinates are in lat or lng)
    zoom: i32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> Option<RenderResult> {
    let (mut left_idx, mut top_idx) = lng_lat_to_tile_x_y(left, top, zoom);
    let (mut right_idx, mut bottom_idx) = lng_lat_to_tile_x_y(right, bottom, zoom);

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

    let render_result = map_render_overlay(map_renderer, &render_area);
    map_renderer.set_current_render_area(render_area);
    Some(render_result)
}

fn map_render_overlay(map_renderer: &MapRenderer, render_area: &RenderArea) -> RenderResult {
    /* for test, map_renderer initialized by MapRenderer::new, tilerenderer size is default size.  */
    let tile_size: u32 = DEFAULT_TILE_SIZE.size(); //512
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

            TileShader::render_on_image(
                &mut image,
                x * tile_size,
                y * tile_size,
                map_renderer.peek_latest_bitmap(),
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
        tile_x_y_to_lng_lat(render_area.left_idx, render_area.top_idx, render_area.zoom);
    let (overlay_right, overlay_bottom) = tile_x_y_to_lng_lat(
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

fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}
fn draw_line3(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(MID_LNG, START_LAT, MID_LNG, END_LAT)
}
fn draw_line4(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, MID_LAT, END_LNG, MID_LAT)
}

pub fn draw_sample_bitmap() -> JourneyBitmap {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    journey_bitmap
}
