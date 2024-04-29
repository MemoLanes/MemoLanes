use chrono::NaiveDateTime;
use hex::ToHex;
use memolanes_core::{gps_processor, journey_bitmap::JourneyBitmap};
use sha1::{Digest, Sha1};
use std::{fs::File, io::Write};

pub fn load_raw_gpx_data_for_test() -> Vec<gps_processor::RawData> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path("./tests/data/raw_gps_shanghai.csv")
        .unwrap();

    let mut data: Vec<gps_processor::RawData> = Vec::new();
    for row in reader.records() {
        let row = row.unwrap();

        let datetime_str = row.get(3).unwrap();
        let datetime_str = &datetime_str[..datetime_str.len() - 3]; // Remove the "+00" offset from the end
        let timestamp_ms = NaiveDateTime::parse_from_str(datetime_str, "%Y/%m/%d %H:%M:%S%.3f")
            .unwrap()
            .timestamp_millis();

        let raw_data = gps_processor::RawData {
            longitude: row.get(0).unwrap().parse().unwrap(),
            latitude: row.get(1).unwrap().parse().unwrap(),
            altitude: Some(row.get(2).unwrap().parse().unwrap()),
            timestamp_ms: Some(timestamp_ms),
            accuracy: Some(row.get(4).unwrap().parse().unwrap()),
            speed: Some(row.get(6).unwrap().parse().unwrap()),
        };
        data.push(raw_data);
    }
    data
}

pub fn assert_image(
    data: &Vec<u8>,
    name_for_inspection_file: &'static str,
    expect_hash: &'static str,
) {
    // for human inspection
    let mut f = File::create(format!(
        "./tests/for_inspection/{}.png",
        name_for_inspection_file
    ))
    .unwrap();
    f.write_all(data).unwrap();
    drop(f);

    // capture image changes
    let mut hasher = Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    assert_eq!(result.encode_hex::<String>(), expect_hash);
}

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;
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

pub fn draw_sample_bitmap() -> JourneyBitmap {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    journey_bitmap
}
