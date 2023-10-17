use chrono::NaiveDateTime;
use native::gps_processor;

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
            latitude: row.get(0).unwrap().parse().unwrap(),
            longitude: row.get(1).unwrap().parse().unwrap(),
            altitude: Some(row.get(2).unwrap().parse().unwrap()),
            timestamp_ms,
            accuracy: row.get(4).unwrap().parse().unwrap(),
            speed: Some(row.get(6).unwrap().parse().unwrap()),
        };
        data.push(raw_data);
    }
    data
}    