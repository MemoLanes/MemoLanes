use bitmap_geometry::*;

#[test]
fn dateline_endpoints_project_to_the_same_polar_coordinates() {
    for (plane, sign) in [
        (BitmapPlane::NorthPolar, 1.0),
        (BitmapPlane::SouthPolar, -1.0),
    ] {
        for latitude in [84.0, MERCATOR_MAX_LATITUDE, 85.052, 87.0, 89.999, 90.0] {
            assert_eq!(
                project_polar_laea(-180.0, sign * latitude, plane),
                project_polar_laea(180.0, sign * latitude, plane),
                "{plane:?}, latitude {latitude}"
            );
        }
    }
}

#[test]
fn poles_have_one_position_regardless_of_longitude() {
    let center = POLAR_DISK_RADIUS as f64;
    for (plane, latitude) in [
        (BitmapPlane::NorthPolar, 90.0),
        (BitmapPlane::SouthPolar, -90.0),
    ] {
        assert_eq!(plane_for_latitude(latitude), plane);
        for longitude in [-180.0, -179.999, -90.0, 0.0, 90.0, 179.999, 180.0] {
            let projected = project_polar_laea(longitude, latitude, plane);
            assert_eq!(projected, (center, center));
            let (lng, lat) = polar_pixel_to_lng_lat(plane, projected.0, projected.1);
            assert!(lng.is_finite()); // Longitude at the pole is not unique.
            assert_eq!(lat, latitude);
        }
    }
}

#[test]
fn dateline_seam_positions_are_exact_disk_boundary_points() {
    let r = POLAR_DISK_RADIUS as f64;
    let w = POLAR_WORLD_PIXEL_WIDTH as f64;
    for longitude in [-180.0, 180.0] {
        assert_eq!(
            project_polar_laea(longitude, MERCATOR_MAX_LATITUDE, BitmapPlane::NorthPolar),
            (r, w)
        );
        assert_eq!(
            project_polar_laea(longitude, -MERCATOR_MAX_LATITUDE, BitmapPlane::SouthPolar),
            (r, 0.0)
        );
    }
}
