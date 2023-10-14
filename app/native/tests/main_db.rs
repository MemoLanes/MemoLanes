use chrono::NaiveDateTime;
use native::{gps_processor, main_db, main_db::MainDb};
use protobuf::Message;
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
    let num_of_gpx_data_in_input = test_data.len();
    print!("total test data: {}\n", num_of_gpx_data_in_input);

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    print!("temp dir: {:?}\n", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    for (i, raw_data) in test_data.iter().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            // test restart
            main_db = MainDb::open(temp_dir.path().to_str().unwrap());
        }
        main_db
            .record(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db.finalize_ongoing_journey().unwrap();

    // validate the finalized journey
    let journeys = main_db.list_all_journeys().unwrap();
    assert_eq!(journeys.len(), 1);
    let journey_id = &journeys[0].id;
    let journey = main_db.get_journey(journey_id).unwrap();
    let num_of_gpx_data = journey.track().track_segmants[0].track_points.len();
    assert_eq!(num_of_gpx_data_in_input, num_of_gpx_data);

    // benefit from zstd
    let data_bytes = journey.write_to_bytes().unwrap();
    let compressed_data_bytes =
        zstd::encode_all(data_bytes.as_slice(), main_db::ZSTD_COMPRESS_LEVEL).unwrap();
    let real_size = data_bytes.len();
    let compressed_size = compressed_data_bytes.len();
    print!(
        "real size: {:.4}MB, compressed size: {:.4}MB\n",
        real_size as f64 / 1024. / 1024.,
        compressed_size as f64 / 1024. / 1024.
    );
    print!(
        "compression rate: {:.2}\n",
        compressed_size as f64 / real_size as f64
    );

    // without any more gpx data, should be no-op
    main_db.finalize_ongoing_journey().unwrap();
    assert_eq!(main_db.list_all_journeys().unwrap().len(), 1);
}

#[test]
fn setting() {
    use main_db::Setting;

    let temp_dir = TempDir::new("main_db-setting").unwrap();
    print!("temp dir: {:?}\n", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    // default value
    assert_eq!(
        main_db.get_setting_with_default(Setting::RawDataMode, false),
        false
    );
    assert_eq!(
        main_db.get_setting_with_default(Setting::RawDataMode, true),
        true
    );

    // setting value
    main_db.set_setting(Setting::RawDataMode, true).unwrap();
    assert_eq!(
        main_db.get_setting_with_default(Setting::RawDataMode, false),
        true
    );

    // restart
    main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    assert_eq!(
        main_db.get_setting_with_default(main_db::Setting::RawDataMode, false),
        true
    );
}
