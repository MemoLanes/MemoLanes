use journey_kernel::journey_bitmap::JourneyBitmap;
use std::fs::File;
use std::io::Write;

#[test]
fn test_serialization() {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);

    // Test binary serialization
    let bytes = journey_bitmap
        .to_bytes()
        .expect("Failed to serialize to bytes");

    // Write bytes to journey_bitmap.bin
    let mut file = File::create("journey_bitmap.bin").expect("Failed to create file");
    file.write_all(&bytes)
        .expect("Failed to write bytes to file");

    let from_bytes = JourneyBitmap::from_bytes(&bytes).expect("Failed to deserialize from bytes");
    assert_eq!(journey_bitmap, from_bytes);
}

const START_LNG: f64 = 151.14;
const START_LAT: f64 = -33.79;
const END_LNG: f64 = 141.27;
const END_LAT: f64 = -25.94;
const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}
fn draw_line3(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(MID_LNG, START_LAT, MID_LNG, END_LAT)
}
fn draw_line4(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, MID_LAT, END_LNG, MID_LAT)
}
