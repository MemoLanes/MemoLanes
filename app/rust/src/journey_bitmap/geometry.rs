//! Geometry shared with the offline geographic rasterizer.

pub use bitmap_geometry::*;

/// Unwrap a projected stroke relative to its previous endpoint. This is a
/// journey-rasterization policy, rather than part of the shared grid contract.
pub(crate) fn unwrap_mercator_x(x: f64, reference: f64) -> f64 {
    let width = MERCATOR_WORLD_PIXEL_WIDTH as f64;
    x + ((reference - x) / width).round() * width
}
