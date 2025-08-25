pub mod test_utils;

use chrono::{DateTime, NaiveDate};
use memolanes_core::{
    gps_processor::{self, Point, RawData},
    import_data,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::JourneyVector,
    main_db::{self, MainDb},
};
use tempdir::TempDir;

#[test]
fn basic() {
    let test_data: Vec<RawData> = import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx")
        .unwrap()
        .into_iter()
        .flatten()
        .collect();
    let num_of_gpx_data_in_input = test_data.len();
    println!("total test data: {num_of_gpx_data_in_input}");

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

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
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();

    // validate the finalized journey
    let journeys = main_db
        .with_txn(|txn| txn.query_journeys(None, None))
        .unwrap();
    assert_eq!(journeys.len(), 1);
    let journey_id = &journeys[0].id;
    let journey_data = main_db
        .with_txn(|txn| txn.get_journey_data(journey_id))
        .unwrap();

    let journey_vector = match &journey_data {
        JourneyData::Vector(vector) => vector,
        JourneyData::Bitmap(_) => panic!("invalid"),
    };
    let num_of_gpx_data = journey_vector.track_segments[0].track_points.len();
    assert_eq!(num_of_gpx_data_in_input, num_of_gpx_data);

    // benefit from zstd
    let mut rough_raw_size: usize = 0;
    for i in &journey_vector.track_segments {
        for _ in &i.track_points {
            rough_raw_size += std::mem::size_of::<f64>() * 2;
        }
    }

    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();
    let compressed_size = buf.len();
    println!(
        "rough size: {:.4}MB, compressed size: {:.4}MB",
        rough_raw_size as f64 / 1024. / 1024.,
        compressed_size as f64 / 1024. / 1024.
    );
    println!(
        "compression rate: {:.2}",
        compressed_size as f64 / rough_raw_size as f64
    );

    // without any more gpx data, should be no-op
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, None))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn setting() {
    use main_db::Setting;

    let temp_dir = TempDir::new("main_db-setting").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    // default value
    assert!(!main_db.get_setting_with_default(Setting::RawDataMode, false));
    assert!(main_db.get_setting_with_default(Setting::RawDataMode, true));

    // setting value
    main_db.set_setting(Setting::RawDataMode, true).unwrap();
    assert!(main_db.get_setting_with_default(Setting::RawDataMode, false));

    // restart
    main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    assert!(main_db.get_setting_with_default(main_db::Setting::RawDataMode, false));
}

#[test]
fn get_ongoing_journey_timestamp_range() {
    let temp_dir = TempDir::new("main_db-get_lastest_timestamp_of_ongoing_journey").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    let result = main_db
        .with_txn(|txn| txn.get_ongoing_journey_timestamp_range())
        .unwrap();
    assert_eq!(result, None);
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349115000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349116000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349117000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    let result = main_db
        .with_txn(|txn| txn.get_ongoing_journey_timestamp_range())
        .unwrap();
    assert_eq!(
        result,
        Some((
            DateTime::from_timestamp(1697349115, 0).unwrap(),
            DateTime::from_timestamp(1697349117, 0).unwrap()
        ))
    );
}

#[test]
fn journey_query() {
    let temp_dir = TempDir::new("main_db-journey_query").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let date = |str| NaiveDate::parse_from_str(str, "%Y-%m-%d").unwrap();

    let add_empty_journey = |txn: &mut main_db::Txn, journey_date_str| {
        txn.create_and_insert_journey(
            date(journey_date_str),
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Vector(JourneyVector {
                track_segments: vec![],
            }),
        )
        .unwrap()
    };

    assert_eq!(
        main_db.with_txn(|txn| txn.earliest_journey_date()).unwrap(),
        None
    );

    main_db
        .with_txn(|txn| {
            add_empty_journey(txn, "2024-08-01");
            add_empty_journey(txn, "2024-08-05");
            add_empty_journey(txn, "2024-08-06");
            add_empty_journey(txn, "2024-08-06");
            add_empty_journey(txn, "2024-08-10");
            add_empty_journey(txn, "2022-06-10");
            add_empty_journey(txn, "2020-05-10");
            add_empty_journey(txn, "2020-02-29"); // leap year
            Ok(())
        })
        .unwrap();

    assert_eq!(
        main_db.with_txn(|txn| txn.earliest_journey_date()).unwrap(),
        Some(date("2020-02-29"))
    );

    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, None))
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(Some(date("2024-08-06")), None))
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(Some(date("2024-08-06")), Some(date("2024-08-06"))))
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, Some(date("2024-08-05"))))
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        main_db.with_txn(|txn| txn.years_with_journey()).unwrap(),
        vec![2020, 2022, 2024]
    );
    // leap year
    assert_eq!(
        main_db
            .with_txn(|txn| txn.months_with_journey(2020))
            .unwrap(),
        vec![2, 5]
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.days_with_journey(2024, 8))
            .unwrap(),
        vec![1, 5, 6, 10]
    );
    // 2-29
    assert_eq!(
        main_db
            .with_txn(|txn| txn.days_with_journey(2020, 2))
            .unwrap(),
        vec![29]
    );
    assert!(main_db
        .with_txn(|txn| txn.days_with_journey(2018, 2))
        .unwrap()
        .is_empty(),);
}
