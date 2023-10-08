use chrono::NaiveDateTime;
use native::{gps_processor, main_db::MainDb};
use tempdir::TempDir;

fn load_raw_gpx_data_for_test() -> Vec<gps_processor::RawData> {
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

#[test]
fn basic() {
    let test_data = load_raw_gpx_data_for_test();
    print!("total test data: {}\n", test_data.len());

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    print!("temp dir: {:?}\n", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    for (i, raw_data) in test_data.iter().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            // test restart
            main_db = MainDb::open(temp_dir.path().to_str().unwrap());
        }
        main_db
            .append_ongoing_journey(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db.finalize_ongoing_journey().unwrap();

    // TODO: load back the finalized journey and make sure it is correct.
}
