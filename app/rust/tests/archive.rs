pub mod test_utils;

use anyhow::Ok;
use chrono::Utc;
use memolanes_core::{
    archive, gps_processor, import_data, journey_data::JourneyData, journey_header::JourneyHeader,
    main_db::MainDb,
};
use std::{fs::File, io::Write};
use tempdir::TempDir;

fn add_vector_journeys(main_db: &mut MainDb) {
    let (raw_data, _preprocessor) =
        import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx").unwrap();

    for (i, raw_data) in raw_data.iter().flatten().enumerate() {
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
            let _id = txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                memolanes_core::journey_header::JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(bitmap),
            )?;
            Ok(())
        })
        .unwrap()
}

fn all_journeys(main_db: &mut MainDb) -> Vec<(JourneyHeader, JourneyData)> {
    let journey_headers = main_db
        .with_txn(|txn| txn.query_journeys(None, None))
        .unwrap();
    let mut journeys = Vec::new();
    for journey_header in journey_headers.into_iter() {
        let journey_data = main_db
            .with_txn(|txn| txn.get_journey_data(&journey_header.id))
            .unwrap();
        journeys.push((journey_header, journey_data));
    }
    journeys
}

#[test]
fn archive_and_import() {
    let temp_dir = TempDir::new("archive-archive_and_import").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);
    let mldx_file_path = temp_dir.path().join("archive.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    main_db
        .with_txn(|txn| archive::export_as_mldx(&archive::WhatToExport::All, txn, &mut file))
        .unwrap();
    drop(file);
    main_db.with_txn(|txn| txn.delete_all_journeys()).unwrap();

    main_db
        .with_txn(|txn| archive::import_mldx(txn, mldx_file_path.to_str().unwrap()))
        .unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn delete_all_journeys() {
    let temp_dir = TempDir::new("archive-delete_all_journeys").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let all_journeys_before = all_journeys(&mut main_db);

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    main_db.with_txn(|txn| txn.delete_all_journeys()).unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn import_broken_archive_and_roll_back() {
    let temp_dir = TempDir::new("archive-import_broken_archive_and_roll_back").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);

    let mldx_file_path = temp_dir.path().join("archive.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    file.write_all("hello".as_bytes()).unwrap();
    drop(file);

    // recover
    assert!(main_db
        .with_txn(|txn| archive::import_mldx(txn, mldx_file_path.to_str().unwrap()))
        .is_err());
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn import_skips_existing_journeys() {
    let temp_dir = TempDir::new("archive-import_skips_existing_journeys").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);
    let mldx_file_path = temp_dir.path().join("archive.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    main_db
        .with_txn(|txn| archive::export_as_mldx(&archive::WhatToExport::All, txn, &mut file))
        .unwrap();
    drop(file);

    // let's delete one journey
    main_db
        .with_txn(|txn| txn.delete_journey(&all_journeys_before[0].0.id))
        .unwrap();
    assert_eq!(
        all_journeys(&mut main_db).len(),
        all_journeys_before.len() - 1
    );

    // import the archive again, it should skip all exisiting journeys but import the deleted one
    main_db
        .with_txn(|txn| archive::import_mldx(txn, mldx_file_path.to_str().unwrap()))
        .unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}
