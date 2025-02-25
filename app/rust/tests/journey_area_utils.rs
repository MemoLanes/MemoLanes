pub mod test_utils;

use memolanes_core::{import_data, journey_area_utils};

//#[test]
fn test_compute_journey_bitmap_area() {
    let (bitmap_import, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let calculated_area = journey_area_utils::compute_journey_bitmap_area(&bitmap_import);
    assert_eq!(calculated_area, 3035670); // area unit: m^2
}
