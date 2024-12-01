pub mod test_utils;

use memolanes_core::{import_data, journey_area_utils};

fn approximate_equal(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn test_get_area_by_journey_bitmap_interation_bit_estimate_block() {
    let (bitmap_import, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    /* area unit: m^2 */
    let expected_area = 3035667.3046264146;
    let calculated_area =
        journey_area_utils::get_area_by_journey_bitmap_interation_bit_estimate_block(
            &bitmap_import,
        )
        .unwrap();
    let tolerance = 1e4;
    let difference = (calculated_area - expected_area).abs();
    dbg!(calculated_area, expected_area, tolerance, difference);
    assert!(
        approximate_equal(calculated_area, expected_area, tolerance),
        "result area {} is not approximately equal to expected area {} ",
        calculated_area,
        expected_area
    );
}
