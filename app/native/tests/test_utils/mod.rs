use chrono::NaiveDateTime;
use hex::ToHex;
use native::gps_processor;
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
            timestamp_ms,
            accuracy: row.get(4).unwrap().parse().unwrap(),
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
