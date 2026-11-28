use anyhow::{ensure, Result};
use geo::BoundingRect;
use geo_data_format::DisplayBounds;
use geo_types::{MultiPolygon, Rect};

use super::ROUNDOFF;

const VIEW_WIDTH: f64 = 640.0;
const VIEW_HEIGHT: f64 = 480.0;
// Match the Explorer's maximum fit: a 512 CSS-pixel MapLibre world at zoom 14.
const MIN_PROJECTED_DEGREES_PER_PIXEL: f64 = 360.0 / (512.0 * (1_u32 << 14) as f64);

fn component_extents(geometry: &MultiPolygon<f64>) -> Vec<Rect<f64>> {
    geometry
        .0
        .iter()
        .filter_map(BoundingRect::bounding_rect)
        .collect()
}

pub(super) fn source_bounds(geometry: &MultiPolygon<f64>) -> DisplayBounds {
    enclosing_bounds(component_extents(geometry))
}

fn enclosing_bounds(mut extents: Vec<Rect<f64>>) -> DisplayBounds {
    extents.sort_by(|a, b| a.min().x.total_cmp(&b.min().x));
    let south = extents.iter().map(|r| r.min().y).fold(90.0, f64::min);
    let north = extents.iter().map(|r| r.max().y).fold(-90.0, f64::max);
    let first = extents[0].min().x;
    let mut reach = extents[0].max().x;
    let mut gap = (0.0, first, first + 360.0);
    // Complement the largest gap in the union of *intervals*, not the set
    // of vertices. A broad polygon occupies all longitudes in its interval.
    for r in &extents[1..] {
        if r.min().x - reach > gap.0 {
            gap = (r.min().x - reach, r.min().x, reach + 360.0);
        }
        reach = reach.max(r.max().x);
    }
    if first + 360.0 - reach >= gap.0 {
        gap = (first + 360.0 - reach, first, reach);
    }
    DisplayBounds {
        west: gap.1,
        south,
        east: gap.2,
        north,
    }
}

pub(super) fn fit_budget(b: DisplayBounds) -> f64 {
    use bitmap_geometry::MERCATOR_MAX_LATITUDE as LIMIT;
    let scale_lat = |lat: f64| lat.clamp(-LIMIT, LIMIT).to_radians();
    let mercator_y = |lat| scale_lat(lat).tan().asinh().to_degrees();
    let fit = ((b.east - b.west) / VIEW_WIDTH)
        .max((mercator_y(b.north) - mercator_y(b.south)) / VIEW_HEIGHT)
        .max(MIN_PROJECTED_DEGREES_PER_PIXEL);
    // Convert the projected pixel to a conservative angular budget using
    // the worst local Mercator scale anywhere in the entity's latitude span.
    fit * scale_lat(b.north.abs().max(b.south.abs())).cos()
        + (b.north - b.south).abs() * f64::EPSILON
}

pub(super) fn validate_camera_bounds(
    geometry: &MultiPolygon<f64>,
    bounds: DisplayBounds,
) -> Result<()> {
    ensure!(
        (-180.0..=180.0).contains(&bounds.west),
        "invalid camera west longitude {}",
        bounds.west
    );
    let extents = component_extents(geometry);
    ensure!(!extents.is_empty(), "display has no polygon extents");
    let tolerance = fit_budget(bounds) + ROUNDOFF;
    let center = (bounds.west + bounds.east) / 2.0;
    // Each cut polygon is continuous in longitude. Place the entire component
    // in one world copy so a broad polygon cannot pass through a camera gap.
    for extent in &extents {
        let shift = 360.0 * ((center - extent.center().x) / 360.0).round();
        ensure!(
            extent.min().x + shift >= bounds.west - tolerance
                && extent.max().x + shift <= bounds.east + tolerance,
            "display polygon lies outside camera longitudes {bounds:?}"
        );
    }
    let actual = enclosing_bounds(extents);
    // Tied gaps may choose different world copies. The minimum spanning width
    // still agrees, within one fit budget at each simplified extreme.
    ensure!(
        ((bounds.east - bounds.west) - (actual.east - actual.west)).abs() <= 2.0 * tolerance
            && (bounds.south - actual.south).abs() <= tolerance
            && (bounds.north - actual.north).abs() <= tolerance,
        "camera bounds {bounds:?} do not fit decoded outline {actual:?}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_camera_fits_both_cut_pieces_after_coordinate_rounding() {
        let pieces = MultiPolygon(vec![
            Rect::new((179.1235, -2.0), (180.0, 3.0)).to_polygon(),
            Rect::new((-180.0, -2.0), (-179.8765, 3.0)).to_polygon(),
        ]);
        let bounds = DisplayBounds {
            west: 179.123456,
            east: 180.123456,
            south: -1.999999,
            north: 3.000001,
        };
        validate_camera_bounds(&pieces, bounds).unwrap();
        let across_the_other_side = DisplayBounds {
            west: -179.876544,
            east: 179.123456,
            ..bounds
        };
        assert!(validate_camera_bounds(&pieces, across_the_other_side).is_err());
    }

    #[test]
    fn fit_budget_floors_subpixel_extents_before_latitude_scale() {
        let bounds = DisplayBounds {
            west: 12.0,
            east: 12.00002,
            south: 40.0,
            north: 40.00002,
        };
        let expected = 360.0 / (512.0 * 2.0_f64.powi(14)) * bounds.north.to_radians().cos()
            + (bounds.north - bounds.south) * f64::EPSILON;
        assert_eq!(fit_budget(bounds), expected);
    }

    #[test]
    fn fit_budget_preserves_larger_projected_extents() {
        for (east, north) in [(20.0, 41.0), (13.0, 50.0)] {
            let bounds = DisplayBounds {
                west: 12.0,
                east,
                south: 40.0,
                north,
            };
            let mercator_y = |lat: f64| lat.to_radians().tan().asinh().to_degrees();
            let previous = ((east - bounds.west) / VIEW_WIDTH)
                .max((mercator_y(north) - mercator_y(bounds.south)) / VIEW_HEIGHT)
                * north.to_radians().cos()
                + (north - bounds.south) * f64::EPSILON;
            assert_eq!(fit_budget(bounds), previous);
        }
    }
}
