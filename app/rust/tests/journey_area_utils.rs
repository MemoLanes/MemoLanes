pub mod test_utils;

use memolanes_core::{import_data, journey_area_utils};
#[macro_use]
extern crate assert_float_eq;

fn f64_to_bits(x: f64) -> u64 {
    x.to_bits()
}

fn get_bit_distance(a: f64, b: f64) -> u64 {
    let a_bits = f64_to_bits(a);
    let b_bits = f64_to_bits(b);
    if a_bits > b_bits {
        a_bits - b_bits
    } else {
        b_bits - a_bits
    }
}

#[test]
fn test_compute_journey_bitmap_area() {
    let (bitmap_import, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let calculated_area = journey_area_utils::compute_journey_bitmap_area(&bitmap_import);
    let bit_distance = get_bit_distance(3035669.991974149, calculated_area);
    let torlerance = 60000;
    dbg!(bit_distance, torlerance);
    assert_f64_near!(calculated_area, 3035669.991974149, torlerance); // area unit: m^2
}
