use geo_rasterizer::projection::{block_area_m2, lng_lat_to_block_xy, BLOCK_GRID_OFFSET};

#[test]
fn equator_origin_maps_to_grid_center() {
    let (x, y) = lng_lat_to_block_xy(0.0, 0.0);
    // Half of 65_536 = 32_768
    assert_eq!(x, 32_768);
    assert_eq!(y, 32_768);
}

#[test]
fn antimeridian_west_edge_is_zero() {
    let (x, _) = lng_lat_to_block_xy(-179.999, 0.0);
    assert_eq!(x, 0);
}

#[test]
fn block_area_at_equator_is_about_374k_m_squared() {
    // ~611.5 m on a side, so ~373_932 m²
    let a = block_area_m2(0, 32_768);
    assert!((a - 374_000.0).abs() < 5_000.0, "got {a}");
}

#[test]
fn block_area_at_60_north_is_smaller() {
    let a_eq = block_area_m2(0, 32_768);
    // Pick a y near 60°N; use lng_lat_to_block_xy to compute it.
    let (_, y_60n) = lng_lat_to_block_xy(0.0, 60.0);
    let a_60 = block_area_m2(0, y_60n as i64);
    assert!(
        a_60 < a_eq * 0.30,
        "60°N block ({a_60}) should be < 30% of equator block ({a_eq})"
    );
}

#[test]
fn block_grid_offset_matches_spec() {
    // MAP_WIDTH_OFFSET (9) + TILE_WIDTH_OFFSET (7) = 16
    assert_eq!(BLOCK_GRID_OFFSET, 16);
}

#[test]
fn round_trip_within_one_block_width() {
    use geo_rasterizer::projection::block_xy_to_lng_lat;
    // For a few sample (lng, lat) points, project to block coords, then back.
    // Result should land within one block of the original (≤ 360°/65536 lng tolerance,
    // ≤ analogous lat tolerance — but lat doesn't flow linearly so we just check
    // that the inverse projects back close, then forward again is identical).
    let samples = [
        (0.0, 0.0),
        (151.2153, -33.8568), // Sydney
        (-73.9855, 40.7580),  // NYC
        (138.7274, 35.3606),  // Mt Fuji
        (2.2945, 48.8584),    // Eiffel Tower
    ];
    for (lng, lat) in samples {
        let (bx, by) = lng_lat_to_block_xy(lng, lat);
        let (lng2, lat2) = block_xy_to_lng_lat(bx as i64, by as i64);
        // Re-project lng2/lat2 — must land in the same block.
        let (bx2, by2) = lng_lat_to_block_xy(lng2, lat2);
        assert_eq!((bx, by), (bx2, by2), "round-trip drift at ({lng}, {lat})");
    }
}

#[test]
fn polar_region_block_area_is_tiny() {
    // 80°N is within Mercator's valid range (~85.05° N/S limit).
    // cos²(80°) ≈ 0.030, so area should be ~3% of equator block.
    let a_eq = block_area_m2(0, 32_768);
    let (_, y_80n) = lng_lat_to_block_xy(0.0, 80.0);
    let a_80 = block_area_m2(0, y_80n as i64);
    assert!(
        a_80 < a_eq * 0.05,
        "80°N block ({a_80}) should be <5% of equator ({a_eq}); cos²(80°)≈0.03"
    );
    assert!(
        a_80 > a_eq * 0.01,
        "80°N block ({a_80}) should be >1% of equator ({a_eq})"
    );
}
