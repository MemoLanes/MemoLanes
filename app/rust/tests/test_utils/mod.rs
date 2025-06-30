use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::renderer::map_renderer::*;
use memolanes_core::utils;
mod render_utils;
use render_utils::*;

use image::RgbaImage;
use serde_json;
use sha2::{Digest, Sha256};
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::{fs::File, io::Write};

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;
const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

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

pub fn verify_image(name: &str, image: &Vec<u8>) {
    let hash_table_path = "tests/image_hashes.lock";
    let mut hash_table: BTreeMap<String, String> = if Path::new(hash_table_path).exists() {
        let hash_table_content =
            fs::read_to_string(hash_table_path).expect("Failed to read hash table file");
        serde_json::from_str(&hash_table_content).unwrap_or_else(|_| BTreeMap::new())
    } else {
        BTreeMap::new()
    };

    // Calculate hash of the current image
    let mut hasher = Sha256::new();
    hasher.update(image);
    let current_hash = format!("{:x}", hasher.finalize());

    if let Some(stored_hash) = hash_table.get(name) {
        // Entry exists, compare hashes
        assert_eq!(
            &current_hash, stored_hash,
            "Image hash mismatch for {name}. Expected: {stored_hash}, Got: {current_hash}. If you have updated the image, please delete the image_hashes.lock file and re-run the tests."
        );
        println!("Verified image hash for: {name}");
    } else {
        // No entry exists, add new entry
        hash_table.insert(name.to_string(), current_hash.clone());
        let hash_table_content =
            serde_json::to_string_pretty(&hash_table).expect("Failed to serialize hash table");
        fs::write(hash_table_path, hash_table_content).expect("Failed to write hash table file");
        println!("Added new hash entry for: {name}");
    }

    // Always save the image file
    let output_path = format!("tests/for_inspection/{name}.png");
    let mut file = File::create(&output_path).expect("Failed to create file");
    file.write_all(image).expect("Failed to write to file");
    println!("Saved image file: {output_path}");
}

pub fn render_map_overlay(
    map_renderer: &MapRenderer,
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
    map_renderer: &MapRenderer,
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
