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
    let angle = lng.to_radians();
    let half = POLAR_WORLD_PIXEL_WIDTH as f64 / 2.0;
    let x = half + half * normalized_radius * angle.sin();
    let oriented_y = half * normalized_radius * angle.cos();
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
