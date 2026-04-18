pub mod test_utils;

use anyhow::Ok;
use chrono::Utc;
use memolanes_core::{
    archive::{self, MldxReader},
    gps_processor, import_data,
    journey_data::JourneyData,
    journey_header::JourneyHeader,
    main_db::MainDb,
};
use std::collections::HashSet;
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

    let mut reader = MldxReader::open(File::open(&mldx_file_path).unwrap()).unwrap();
    main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
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

    // delete one journey
    main_db
        .with_txn(|txn| txn.delete_journey(&all_journeys_before[0].0.id))
        .unwrap();
    assert_eq!(
        all_journeys(&mut main_db).len(),
        all_journeys_before.len() - 1
    );

    // analyze: existing journeys = skipped, deleted one = new; import only new
    let mut reader = MldxReader::open(File::open(&mldx_file_path).unwrap()).unwrap();
    main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn import_selected_journeys_by_id() {
    let temp_dir = TempDir::new("archive-import_selected_journeys_by_id").unwrap();
    let mut source_db = MainDb::open(temp_dir.path().to_str().unwrap());
    add_vector_journeys(&mut source_db);
    add_bitmap_journey(&mut source_db);
    let all_from_archive = all_journeys(&mut source_db);

    let mldx_file_path = temp_dir.path().join("selected-import.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    source_db
        .with_txn(|txn| archive::export_as_mldx(&archive::WhatToExport::All, txn, &mut file))
        .unwrap();
    drop(file);

    let target_dir = TempDir::new("archive-selected-target").unwrap();
    let mut target_db = MainDb::open(target_dir.path().to_str().unwrap());

    let selected_id = all_from_archive[0].0.id.clone();
    let mut selected_ids = HashSet::new();
    selected_ids.insert(selected_id.clone());

    let mut reader = MldxReader::open(File::open(&mldx_file_path).unwrap()).unwrap();
    let import_result = target_db
        .with_txn(|txn| reader.import(txn, Some(&selected_ids)))
        .unwrap();
    assert_eq!(import_result.imported_count, 1);
    assert_eq!(
        import_result.ignored_by_filter_count as usize,
        all_from_archive.len() - 1
    );

    let imported = all_journeys(&mut target_db);
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].0.id, selected_id);
}
