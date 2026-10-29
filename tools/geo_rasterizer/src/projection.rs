//! WGS84 ↔ block coordinate projection and prepared scan-converter geometry.
//!
//! Mercator conventions mirror `app/rust/src/utils.rs`; the area formula
//! mirrors `mercator_block_area_cm2` in `app/rust/src/journey_bitmap/area.rs`.
//! The offline tool uses block-grid resolution and avoids a dependency on the
//! full runtime crate.

use std::f64::consts::PI;

use bitmap_geometry::EARTH_RADIUS_METERS as EARTH_RADIUS_M;
use geo::BoundingRect;
use geo_types::{Coord, MultiPolygon, Rect};

/// Slippy-map zoom level at the block-grid resolution.
pub const BLOCK_GRID_OFFSET: i32 =
    (bitmap_geometry::MAP_WIDTH_OFFSET + bitmap_geometry::TILE_WIDTH_OFFSET) as i32;
/// Block-grid resolution per side (2^16).
pub const BLOCK_GRID_SIZE: i64 = 1 << BLOCK_GRID_OFFSET;

/// One vertex in block space, ready for both scanline fill and DDA.
pub(crate) struct BlockVertex {
    pub position: Coord<f64>,
    pub dda_cell: Coord<i64>,
}

/// Prepared block-space geometry. The scan converter never receives lng/lat.
pub(crate) struct BlockGeometry {
    pub rings: Vec<Vec<BlockVertex>>,
    pub bounds: Rect<u32>,
}

impl BlockGeometry {
    pub(crate) fn prepare_mercator(lnglat: &MultiPolygon<f64>) -> Option<Self> {
        let bounds = lnglat.bounding_rect()?;
        let min = lng_lat_to_block_xy(bounds.min().x, bounds.max().y);
        let max = lng_lat_to_block_xy(bounds.max().x, bounds.min().y);
        Some(Self::prepare(lnglat, Rect::new(min, max), |point| {
            let (x, y) = lng_lat_to_block_xy_float(point.x, point.y);
            // Preserve the legacy integer projection and endpoint clamping.
            // Flooring the float position can instead select the grid width
            // at the dateline or Mercator seam.
            let (cell_x, cell_y) = lng_lat_to_block_xy(point.x, point.y);
            BlockVertex {
                position: Coord { x, y },
                dda_cell: Coord {
                    x: i64::from(cell_x),
                    y: i64::from(cell_y),
                },
            }
        }))
    }

    pub(crate) fn from_projected_blocks(
        blocks: &MultiPolygon<f64>,
        grid_size: i64,
    ) -> Option<Self> {
        let bounds = blocks.bounding_rect()?;
        let clamp = |v: f64| v.floor().clamp(0.0, (grid_size - 1) as f64) as u32;
        Some(Self::prepare(
            blocks,
            Rect::new(
                (clamp(bounds.min().x), clamp(bounds.min().y)),
                (clamp(bounds.max().x), clamp(bounds.max().y)),
            ),
            |position| BlockVertex {
                position,
                // Keep polar endpoints outside the grid: DDA discards their
                // out-of-grid samples rather than moving them onto its edge.
                dda_cell: Coord {
                    x: position.x.floor() as i64,
                    y: position.y.floor() as i64,
                },
            },
        ))
    }

    fn prepare(
        geometry: &MultiPolygon<f64>,
        bounds: Rect<u32>,
        project: impl Fn(Coord<f64>) -> BlockVertex,
    ) -> Self {
        let rings = geometry
            .0
            .iter()
            .flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors()))
            .map(|ring| ring.0.iter().copied().map(&project).collect())
            .collect();
        Self { rings, bounds }
    }
}

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

/// Float block positions used for scanline intersections, before flooring.
fn lng_lat_to_block_xy_float(lng: f64, lat: f64) -> (f64, f64) {
    let n = BLOCK_GRID_SIZE as f64;
    let lat_rad = lat.to_radians();
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (
        x.clamp(0.0, n - f64::EPSILON),
        y.clamp(0.0, n - f64::EPSILON),
    )
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
/// Trapezoidal spherical-area formula also used by `mercator_block_area_cm2`
/// in `app/rust/src/journey_bitmap/area.rs`. The runtime evaluates it on native
/// pixels; this function covers a complete block at `BLOCK_GRID_OFFSET`.
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
