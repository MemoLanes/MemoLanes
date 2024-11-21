pub mod test_utils;

use memolanes_core::{
    import_data, journey_area_utils,
};

fn approximate_equal(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn import_and_calculate_journey_bitmap_area() {
    let (bitmap_import, warnings) =
        import_data::load_fow_sync_data("./tests/data/Sync-used-for-fm-screenshot.zip").unwrap();
    /* 43.412037048499996 km^2 */
    let expected_area = 43412037.048499996;
    let calculated_area = journey_area_utils::get_area_by_journey_bitmap(&bitmap_import).unwrap();
    let tolerance = 1e4;
    dbg!(calculated_area, expected_area, tolerance);
    assert!(
        approximate_equal(calculated_area, expected_area, tolerance),
        "result area {} is not approximately equal to expected area {} ",
        calculated_area,
        expected_area
    );
}
