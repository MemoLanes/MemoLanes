use bitmap_geometry::*;

#[test]
fn pixel_area_quantization_matches_the_projection_scale() {
    let raw_m2 = polar_pixel_size_meters().powi(2);
    // Independent 70-digit evaluation of the spherical LAEA scale.
    assert!((raw_m2 - 1.100_962_165_765_832).abs() < 1e-14);
    assert_eq!(polar_pixel_area_cm2(), 11_010);
    assert!((polar_pixel_area_cm2() as f64 - raw_m2 * 10_000.0).abs() <= 0.5);
}

#[test]
fn boundary_pixels_match_high_precision_analytic_fixtures() {
    // Circle/rectangle intersections evaluated at 70 decimal digits using
    // F(t) = (t*sqrt(R²-t²) + R²*asin(t/R))/2, not Simpson integration.
    // Include near-tangent cells and values close to half-cm² rounding thresholds.
    let fixtures = [
        (524288, 0, 0.999_999_682_108_561_2, 11010),
        (524351, 0, 0.996_154_467_250_707, 10968),
        (524800, 0, 0.749_511_341_020_602, 8252),
        (525311, 0, 0.000_975_293_026_089_290_9, 11),
        (525312, 0, 0.0, 0),
        (524288, 524288, 1.0, 11010),
        (0, 0, 0.0, 0),
        (904810, 884955, 0.048_001_813_400_534_43, 528),
        (910407, 169620, 0.982_697_551_300_037_4, 10820),
        (128851, 180037, 0.042_188_929_017_942_25, 465),
        (1048373, 509728, 0.802_043_583_892_347_6, 8830),
        (1048109, 546401, 0.439_100_831_876_358_7, 4835),
        (562539, 1397, 0.743_551_333_313_052, 8187),
        (305158, 1000586, 0.403_496_850_940_415_1, 4443),
        (705780, 32415, 0.320_208_868_147_857_5, 3525),
        (408253, 13001, 0.516_303_395_560_825_7, 5685),
        (225786, 955304, 0.069_618_565_411_902_8, 767),
        (285944, 991267, 0.914_577_696_177_464_3, 10070),
        (725476, 1008438, 0.006_857_449_279_742_712, 76),
    ];
    let last = POLAR_WORLD_PIXEL_WIDTH - 1;
    for (x, y, expected_fraction, expected_cm2) in fixtures {
        for (px, py) in [
            (x, y),
            (last - x, y),
            (x, last - y),
            (last - x, last - y),
            (y, x),
            (last - y, x),
            (y, last - x),
            (last - y, last - x),
        ] {
            let fraction = polar_pixel_fraction(px, py);
            assert!(fraction.is_finite() && (0.0..=1.0).contains(&fraction));
            assert!((fraction - expected_fraction).abs() < 1e-9, "({px}, {py})");
            assert_eq!(
                (fraction * polar_pixel_area_cm2() as f64).round() as i64,
                expected_cm2,
                "({px}, {py})"
            );
            assert_eq!(
                polar_rectangle_area_cm2(px..px + 1, py..py + 1),
                expected_cm2
            );
        }
    }
}

#[test]
fn rectangles_round_per_pixel_not_after_summing_fractions() {
    let x = POLAR_DISK_RADIUS + 4;
    let unit_area = polar_pixel_area_cm2() as f64;
    let a = polar_pixel_fraction(x, 0) * unit_area;
    let b = polar_pixel_fraction(x + 1, 0) * unit_area;
    assert_eq!(a.round() as i64 + b.round() as i64, 22_020);
    assert_eq!((a + b).round() as i64, 22_019);
    assert_eq!(polar_rectangle_area_cm2(x..x + 2, 0..1), 22_020);
}

#[test]
fn rectangles_match_pixel_sums_and_are_exactly_additive() {
    let r = POLAR_DISK_RADIUS as f64;
    let angles = (0..32)
        .map(|i| std::f64::consts::TAU * (i as f64 + 0.25) / 32.0)
        // Include explicit axial and near-axis boundary cases.
        .chain([0.0, 0.01, 0.3, 0.78, 1.2, 1.56]);
    for angle in angles {
        for size in [1, 2, 3, 7, 16, 64, 129] {
            let x = ((r + r * angle.cos()).floor() as i64 - size / 2)
                .clamp(0, POLAR_WORLD_PIXEL_WIDTH - size);
            let y = ((r + r * angle.sin()).floor() as i64 - size / 2)
                .clamp(0, POLAR_WORLD_PIXEL_WIDTH - size);
            let mut expected = 0;
            for px in x..x + size {
                for py in y..y + size {
                    expected += (polar_pixel_fraction(px, py) * polar_pixel_area_cm2() as f64)
                        .round() as i64;
                }
            }
            let area = polar_rectangle_area_cm2(x..x + size, y..y + size);
            assert_eq!(area, expected, "({x}, {y}), size {size}");
            if size > 1 {
                let mid_x = x + size / 2;
                let mid_y = y + size / 2;
                assert_eq!(
                    area,
                    polar_rectangle_area_cm2(x..mid_x, y..y + size)
                        + polar_rectangle_area_cm2(mid_x..x + size, y..y + size)
                );
                assert_eq!(
                    area,
                    polar_rectangle_area_cm2(x..x + size, y..mid_y)
                        + polar_rectangle_area_cm2(x..x + size, mid_y..y + size)
                );
            }
        }
    }
}
