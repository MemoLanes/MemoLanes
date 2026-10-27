use bitmap_geometry::*;

#[test]
fn polar_roundtrips_and_seams() {
    for plane in [BitmapPlane::NorthPolar, BitmapPlane::SouthPolar] {
        let sign = if plane == BitmapPlane::NorthPolar {
            1.0
        } else {
            -1.0
        };
        let r = POLAR_DISK_RADIUS as f64;
        for lng in [-180.0, -91.0, 0.0, 73.0, 179.99] {
            for latitude in [MERCATOR_MAX_LATITUDE, 87.0, 89.999] {
                let lat = sign * latitude;
                let (x, y) = project_polar_laea(lng, lat, plane);
                let (back_lng, back_lat) = polar_pixel_to_lng_lat(plane, x, y);
                assert!((back_lat - lat).abs() < 1e-10);
                let delta = (back_lng - lng + 180.0).rem_euclid(360.0) - 180.0;
                assert!(delta.abs() < 1e-8);
                if latitude == MERCATOR_MAX_LATITUDE {
                    assert!(((x - r).hypot(y - r) - r).abs() < 1e-8);
                }
            }
        }
        assert_eq!(project_polar_laea(30.0, sign * 90.0, plane), (r, r));
    }
}

#[test]
fn disk_clipping_retains_intersecting_cells_and_excludes_corners() {
    let r = POLAR_DISK_RADIUS;
    let (near, _) = polar_rect_radius_bounds_squared(0..64, 0..64);
    assert!(near >= r * r);
    let (near, far) = polar_rect_radius_bounds_squared(r..r + 64, 0..64);
    assert!(near < r * r && far > r * r);
    assert!(polar_row_span(0).contains(&r));
    assert_eq!(
        block_pixel_bounds((1, 2), (3, 4)),
        (8384..8448, 16640..16704)
    );
}

#[test]
fn rectangle_area_matches_individually_quantized_pixels() {
    let r = POLAR_DISK_RADIUS;
    for angle in [0.0_f64, 0.01, 0.3, 0.78, 1.2, 1.56] {
        let x = r + (r as f64 * angle.cos()) as i64 - 32;
        let y = r + (r as f64 * angle.sin()) as i64 - 32;
        let xr = x.clamp(0, 2 * r - 64)..(x.clamp(0, 2 * r - 64) + 64);
        let yr = y.clamp(0, 2 * r - 64)..(y.clamp(0, 2 * r - 64) + 64);
        let mut expected = 0;
        for px in xr.clone() {
            for py in yr.clone() {
                expected +=
                    (polar_pixel_fraction(px, py) * polar_pixel_area_cm2() as f64).round() as i64;
            }
        }
        assert_eq!(polar_rectangle_area_cm2(xr, yr), expected, "angle {angle}");
    }
}
