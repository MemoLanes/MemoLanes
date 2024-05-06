pub mod test_utils;

use chrono::Utc;
use memolanes_core::{
    archive, gps_processor, import_data, journey_data::JourneyData, journey_header::JourneyHeader,
    main_db::MainDb,
};
use std::{fs::File, io::Write};
use tempdir::TempDir;

fn add_vector_journeys(main_db: &mut MainDb) {
    let test_data = test_utils::load_raw_gpx_data_for_test();
    for (i, raw_data) in test_data.iter().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            main_db
                .with_txn(|txn| txn.finalize_ongoing_journey())
                .unwrap();
        }
        main_db
            .record(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();
}

fn add_bitmap_journey(main_db: &mut MainDb) {
    let (bitmap, _warnings) = import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                memolanes_core::journey_header::JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(bitmap),
            )
        })
        .unwrap()
}

fn all_journeys(main_db: &mut MainDb) -> Vec<(JourneyHeader, JourneyData)> {
    let journey_headers = main_db.with_txn(|txn| txn.list_all_journeys()).unwrap();
    let mut journeys = Vec::new();
    for journey_header in journey_headers.into_iter() {
        let journey_data = main_db
            .with_txn(|txn| txn.get_journey(&journey_header.id))
            .unwrap();
        journeys.push((journey_header, journey_data));
    }
    journeys
}

#[test]
fn archive_and_recover() {
    let temp_dir = TempDir::new("main_db-basic").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);

    let zip_file_path = temp_dir.path().join("archive.zip");
    let mut file = File::create(&zip_file_path).unwrap();
    main_db
        .with_txn(|txn| archive::archive_all_as_zip(txn, &mut file))
        .unwrap();
    drop(file);

    // Do something to change things in `main_db`.
    add_bitmap_journey(&mut main_db);
    assert_ne!(all_journeys_before, all_journeys(&mut main_db));

    // recover
    main_db
        .with_txn(|txn| archive::recover_archive_file(txn, zip_file_path.to_str().unwrap()))
        .unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn recover_from_broken_archive_and_roll_back() {
    let temp_dir = TempDir::new("main_db-basic").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);

    let zip_file_path = temp_dir.path().join("archive.zip");
    let mut file = File::create(&zip_file_path).unwrap();
    file.write_all("hello".as_bytes()).unwrap();
    drop(file);

    // recover
    assert!(main_db
        .with_txn(|txn| archive::recover_archive_file(txn, zip_file_path.to_str().unwrap()))
        .is_err());
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}
