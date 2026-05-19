//! WGS84 ↔ block coordinate projection. Mirrors
//! `app/rust/src/utils.rs` (`lng_lat_to_tile_x_y`, `tile_x_y_to_lng_lat`)
//! and the trapezoidal area formula in
//! `app/rust/src/journey_area_utils.rs:38-46`. Duplicated here because
//! `memolanes_core` is too heavy a dep for the offline rasterizer.
//!
//! Constants must stay in lockstep with `journey_bitmap.rs`:
//!   MAP_WIDTH_OFFSET = 9, TILE_WIDTH_OFFSET = 7.
//! BLOCK_GRID_OFFSET = MAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET = 16.
//! Block grid resolution: 2^16 = 65_536 per side.

use std::f64::consts::PI;

/// Slippy-map zoom level at the block-grid resolution.
pub const BLOCK_GRID_OFFSET: i32 = 16;
/// Block-grid resolution per side (2^16).
pub const BLOCK_GRID_SIZE: i64 = 1 << BLOCK_GRID_OFFSET;
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Project (lng, lat) to integer block coordinates in [0, BLOCK_GRID_SIZE).
/// Equivalent to `lng_lat_to_tile_x_y(lng, lat, 16)` from `app/rust/src/utils.rs`.
pub fn lng_lat_to_block_xy(lng: f64, lat: f64) -> (u32, u32) {
    let n = f64::powi(2.0, BLOCK_GRID_OFFSET);
    let lat_rad = lat.to_radians();
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    let x = x.floor().clamp(0.0, n - 1.0) as u32;
    let y = y.floor().clamp(0.0, n - 1.0) as u32;
    (x, y)
}

/// Inverse of `lng_lat_to_block_xy`. Returns the lng/lat at block
/// (x, y)'s top-left corner.
pub fn block_xy_to_lng_lat(x: i64, y: i64) -> (f64, f64) {
    debug_assert!(
        (0..=BLOCK_GRID_SIZE).contains(&x) && (0..=BLOCK_GRID_SIZE).contains(&y),
        "block_xy_to_lng_lat: ({x}, {y}) out of range [0, {BLOCK_GRID_SIZE}]"
    );
    let n = f64::powi(2.0, BLOCK_GRID_OFFSET);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(PI * (1.0 - (2.0 * y as f64) / n))) * 180.0) / PI;
    (lng, lat)
}

/// Earth-surface area of a single block at integer coords (x, y), in m².
/// Trapezoidal approximation matching `compute_one_tile` in
/// `app/rust/src/journey_area_utils.rs:38-46`.
/// Note: `compute_one_tile` operates at bit-grid resolution (zoom 22);
/// `block_area_m2` operates at block-grid resolution (zoom 16). The
/// trapezoidal formula is identical.
pub fn block_area_m2(x: i64, y: i64) -> f64 {
    debug_assert!(
        (0..BLOCK_GRID_SIZE).contains(&x) && (0..BLOCK_GRID_SIZE).contains(&y),
        "block_area_m2: ({x}, {y}) out of range [0, {BLOCK_GRID_SIZE})"
    );
    let (lng1, lat1) = block_xy_to_lng_lat(x, y);
    let (lng2, lat2) = block_xy_to_lng_lat(x + 1, y + 1);
    let d_lambda = (lng2 - lng1).abs().to_radians();
    let width_top = EARTH_RADIUS_M * d_lambda * lat1.to_radians().cos();
    let width_bottom = EARTH_RADIUS_M * d_lambda * lat2.to_radians().cos();
    let avg_width = (width_top + width_bottom) / 2.0;
    let height = EARTH_RADIUS_M * (lat2 - lat1).abs().to_radians();
    avg_width * height
}
