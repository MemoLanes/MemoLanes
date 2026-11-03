//! Shared journey/region grid geometry, depending only on the standard library.
//! Keeps geographic assets and runtime bitmaps on exactly the same projections
//! and cell boundaries. Serialization, polygon processing and journey storage
//! belong to the consuming crates, not this one.
//!
//! Native pixel (x, y) occupies [x, x+1) × [y, y+1). Each polar square
//! contains an LAEA disk centred at (R, R); its circumference is the Mercator
//! latitude seam. Projection extends beyond the disk so callers can clip
//! footprints without clamping their centres.

use std::ops::Range;

pub const EARTH_RADIUS_METERS: f64 = 6_371_000.0;
pub const TILE_WIDTH_OFFSET: i16 = 7;
pub const MAP_WIDTH_OFFSET: i16 = 9;
pub const MAP_WIDTH: i64 = 1 << MAP_WIDTH_OFFSET;
pub const TILE_WIDTH: i64 = 1 << TILE_WIDTH_OFFSET;
pub const BITMAP_WIDTH_OFFSET: i16 = 6;
pub const BITMAP_WIDTH: i64 = 1 << BITMAP_WIDTH_OFFSET;
pub const MERCATOR_WORLD_PIXEL_OFFSET: i16 =
    MAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;
pub const MERCATOR_WORLD_PIXEL_WIDTH: i64 = 1 << MERCATOR_WORLD_PIXEL_OFFSET;
pub const EQUATORIAL_PIXEL_SIZE_METERS: f64 =
    std::f64::consts::TAU * EARTH_RADIUS_METERS / MERCATOR_WORLD_PIXEL_WIDTH as f64;
pub const MERCATOR_MAX_LATITUDE: f64 = 85.051_128_779_806_6;

/// Polar squares are 2^20 pixels wide (roughly one metre per pixel).
pub const POLAR_MAP_WIDTH_OFFSET: i16 = 7;
pub const POLAR_MAP_WIDTH: i64 = 1 << POLAR_MAP_WIDTH_OFFSET;
const POLAR_WORLD_PIXEL_OFFSET: i16 =
    POLAR_MAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET;
pub const POLAR_WORLD_PIXEL_WIDTH: i64 = 1 << POLAR_WORLD_PIXEL_OFFSET;
pub const POLAR_DISK_RADIUS: i64 = POLAR_WORLD_PIXEL_WIDTH / 2;

/// Projection identity. Numeric identifiers are stable for serialized consumers.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum BitmapPlane {
    Mercator = 0,
    NorthPolar = 1,
    SouthPolar = 2,
}

impl BitmapPlane {
    pub const ALL: [Self; 3] = [Self::Mercator, Self::NorthPolar, Self::SouthPolar];

    pub fn source_tile_zoom(self) -> i16 {
        match self {
            Self::Mercator => MAP_WIDTH_OFFSET,
            Self::NorthPolar | Self::SouthPolar => POLAR_MAP_WIDTH_OFFSET,
        }
    }

    pub fn grid_width(self) -> i64 {
        1 << self.source_tile_zoom()
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Mercator),
            1 => Some(Self::NorthPolar),
            2 => Some(Self::SouthPolar),
            _ => None,
        }
    }
}

pub fn project_mercator(lng: f64, lat: f64) -> (f64, f64) {
    let width = MERCATOR_WORLD_PIXEL_WIDTH as f64;
    let lat = lat.to_radians();
    (
        (lng / 360.0 + 0.5) * width,
        (0.5 - (std::f64::consts::FRAC_PI_4 + lat * 0.5).tan().ln() / std::f64::consts::TAU)
            * width,
    )
}

pub fn plane_for_latitude(latitude: f64) -> BitmapPlane {
    if latitude > MERCATOR_MAX_LATITUDE {
        BitmapPlane::NorthPolar
    } else if latitude < -MERCATOR_MAX_LATITUDE {
        BitmapPlane::SouthPolar
    } else {
        BitmapPlane::Mercator
    }
}

fn polar_seam_radius() -> f64 {
    let colatitude = (90.0 - MERCATOR_MAX_LATITUDE).to_radians();
    2.0 * (colatitude / 2.0).sin()
}

pub fn polar_pixel_size_meters() -> f64 {
    2.0 * EARTH_RADIUS_METERS * polar_seam_radius() / POLAR_WORLD_PIXEL_WIDTH as f64
}

pub fn project_polar_laea(lng: f64, lat: f64, plane: BitmapPlane) -> (f64, f64) {
    let signed_lat = match plane {
        BitmapPlane::NorthPolar => lat,
        BitmapPlane::SouthPolar => -lat,
        BitmapPlane::Mercator => unreachable!(),
    };
    let colatitude = (90.0 - signed_lat).to_radians();
    let rho = 2.0 * (colatitude / 2.0).sin();
    let normalized_radius = rho / polar_seam_radius();
    // sin(±π) is not exactly zero in floating point. Keep both dateline
    // representations on the same native grid axis before flooring to cells.
    let (sin_lng, cos_lng) = if lng.abs() == 180.0 {
        (0.0, -1.0)
    } else {
        let angle = lng.to_radians();
        (angle.sin(), angle.cos())
    };
    let half = POLAR_WORLD_PIXEL_WIDTH as f64 / 2.0;
    let x = half + half * normalized_radius * sin_lng;
    let oriented_y = half * normalized_radius * cos_lng;
    let y = match plane {
        BitmapPlane::NorthPolar => half - oriented_y,
        BitmapPlane::SouthPolar => half + oriented_y,
        BitmapPlane::Mercator => unreachable!(),
    };
    (x, y)
}

pub fn polar_pixel_to_lng_lat(plane: BitmapPlane, x: f64, y: f64) -> (f64, f64) {
    let half = POLAR_DISK_RADIUS as f64;
    let x = x - half;
    let y = match plane {
        BitmapPlane::NorthPolar => half - y,
        BitmapPlane::SouthPolar => y - half,
        BitmapPlane::Mercator => unreachable!(),
    };
    let rho = x.hypot(y) / half * polar_seam_radius();
    let latitude = 90.0 - 2.0 * (rho * 0.5).asin().to_degrees();
    (
        x.atan2(y).to_degrees(),
        if plane == BitmapPlane::NorthPolar {
            latitude
        } else {
            -latitude
        },
    )
}

pub fn block_pixel_bounds(tile: (u16, u16), block: (u8, u8)) -> (Range<i64>, Range<i64>) {
    let x = (i64::from(tile.0) * TILE_WIDTH + i64::from(block.0)) * BITMAP_WIDTH;
    let y = (i64::from(tile.1) * TILE_WIDTH + i64::from(block.1)) * BITMAP_WIDTH;
    (x..x + BITMAP_WIDTH, y..y + BITMAP_WIDTH)
}

/// Squared nearest/farthest distances from the polar centre to a rectangle.
/// near >= R² means no positive-area intersection; far <= R² means full inclusion.
pub fn polar_rect_radius_bounds_squared(x: Range<i64>, y: Range<i64>) -> (i64, i64) {
    debug_assert!(!x.is_empty() && !y.is_empty());
    let radius = POLAR_DISK_RADIUS;
    let near_x = (x.start - radius).max(radius - x.end).max(0);
    let near_y = (y.start - radius).max(radius - y.end).max(0);
    let far_x = (x.start - radius).abs().max((x.end - radius).abs());
    let far_y = (y.start - radius).abs().max((y.end - radius).abs());
    (
        near_x * near_x + near_y * near_y,
        far_x * far_x + far_y * far_y,
    )
}

/// Cells in a native pixel row with a positive-area disk intersection.
pub fn polar_row_span(row: i64) -> Range<i64> {
    debug_assert!((0..POLAR_WORLD_PIXEL_WIDTH).contains(&row));
    let radius = POLAR_DISK_RADIUS;
    let near_y = (row - radius).max(radius - row - 1).max(0);
    let extent = ((radius * radius - near_y * near_y) as f64).sqrt();
    let start = (radius as f64 - extent).floor() as i64;
    let end = (radius as f64 + extent).ceil() as i64;
    start..end
}

/// Quantize each pixel independently, matching journey-area accumulation.
pub fn polar_pixel_area_cm2() -> i64 {
    let side = polar_pixel_size_meters();
    (side * side * 10_000.0).round() as i64
}

/// Fraction of a native pixel inside the cap.
pub fn polar_pixel_fraction(x: i64, y: i64) -> f64 {
    let (near, far) = polar_rect_radius_bounds_squared(x..x + 1, y..y + 1);
    let r2 = POLAR_DISK_RADIUS * POLAR_DISK_RADIUS;
    if near >= r2 {
        return 0.0;
    }
    if far <= r2 {
        return 1.0;
    }
    let x = x - POLAR_DISK_RADIUS;
    let y = y - POLAR_DISK_RADIUS;
    // Reflect into the first octant to keep the circle's slope at most ~1.
    let x = x.abs().min((x + 1).abs()) as f64;
    let y = y.abs().min((y + 1).abs()) as f64;
    let (x, y) = (x.min(y), x.max(y));
    let radius = POLAR_DISK_RADIUS as f64;
    let circle = |t: f64| ((radius - t) * (radius + t)).max(0.0).sqrt();
    let full_end = circle(y + 1.0).clamp(x, x + 1.0);
    let arc_end = circle(y).clamp(full_end, x + 1.0);
    let height = |t| circle(t) - y;
    // Simpson integration over at most one native pixel; error is below cm²
    // quantization without subtracting large circle sectors.
    let curved = (arc_end - full_end)
        * (height(full_end) + 4.0 * height((full_end + arc_end) * 0.5) + height(arc_end))
        / 6.0;
    (full_end - x + curved).clamp(0.0, 1.0)
}

/// Area of a completely filled native rectangle, clipped to the cap. Whole
/// interiors are O(1); only edge pixels need integration and cm² rounding.
pub fn polar_rectangle_area_cm2(x: Range<i64>, y: Range<i64>) -> i64 {
    let (near, far) = polar_rect_radius_bounds_squared(x.clone(), y.clone());
    let r = POLAR_DISK_RADIUS;
    let bit_area = polar_pixel_area_cm2();
    if near >= r * r {
        return 0;
    }
    if far <= r * r {
        return (x.end - x.start) * (y.end - y.start) * bit_area;
    }
    let mut area = 0;
    for row in y {
        let span = polar_row_span(row);
        let start = x.start.max(span.start);
        let end = x.end.min(span.end);
        if start >= end {
            continue;
        }
        let far_y = (row - r).abs().max((row + 1 - r).abs());
        let extent = ((r * r - far_y * far_y) as f64).sqrt();
        let full_start = ((r as f64 - extent).ceil() as i64).clamp(start, end);
        let full_end = ((r as f64 + extent).floor() as i64).clamp(full_start, end);
        area += (full_end - full_start) * bit_area;
        for col in (start..full_start).chain(full_end..end) {
            area += (polar_pixel_fraction(col, row) * bit_area as f64).round() as i64;
        }
    }
    area
}
