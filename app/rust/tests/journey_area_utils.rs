pub mod test_utils;

use memolanes_core::{import_data, journey_area_utils};
#[macro_use]
extern crate assert_float_eq;

#[test]
fn test_compute_journey_bitmap_area() {
    let (bitmap_import, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    /* area unit: m^2 */
    let calculated_area = journey_area_utils::compute_journey_bitmap_area(&bitmap_import);
    assert_f64_near!(calculated_area, 3035669.9919493743);
}
