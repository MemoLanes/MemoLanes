pub mod test_utils;

use anyhow::Ok;
use chrono::{DateTime, NaiveDate, Utc};
use memolanes_core::{
    archive::{self, MldxReader, SectionVersion},
    gps_processor, import_data,
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind, JourneyType},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::MainDb,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::Cursor;
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

fn sample_journey() -> (JourneyHeader, JourneyData) {
    let ts = |sec| DateTime::from_timestamp(sec, 0).unwrap();
    let header = JourneyHeader {
        id: "test-journey-id".to_owned(),
        revision: "test-revision".to_owned(),
        journey_date: NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
        created_at: ts(1),
        updated_at: Some(ts(2)),
        start: Some(ts(3)),
        end: Some(ts(4)),
        journey_type: JourneyType::Vector,
        journey_kind: JourneyKind::DefaultKind,
        note: Some("test note".to_owned()),
        postprocessor_algo: None,
    };
    let data = JourneyData::Vector(JourneyVector {
        track_segments: vec![TrackSegment {
            track_points: vec![
                TrackPoint {
                    latitude: 31.2304,
                    longitude: 121.4737,
                },
                TrackPoint {
                    latitude: 31.2314,
                    longitude: 121.4747,
                },
            ],
        }],
    });
    (header, data)
}

fn write_single_journey_archive(
    section_version: SectionVersion,
) -> (Vec<u8>, JourneyHeader, JourneyData) {
    let (header, data) = sample_journey();
    let mut writer = Cursor::new(Vec::new());
    archive::export_single_journey_as_mldx(
        header.clone(),
        data.clone(),
        &mut writer,
        section_version,
    )
    .unwrap();
    (writer.into_inner(), header, data)
}

fn expected_metadata_name(section_version: SectionVersion) -> &'static str {
    match section_version {
        SectionVersion::V1 => "metadata.xxm",
        SectionVersion::V2 => "metadata.mldm",
    }
}

fn metadata_entry_name(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    for index in 0..zip.len() {
        let name = zip.by_index(index).unwrap().name().to_owned();
        if name.starts_with("metadata.") {
            return name;
        }
    }
    panic!("missing metadata entry")
}

fn rewrite_metadata_entry_name(bytes: Vec<u8>, metadata_name: &str) -> Vec<u8> {
    let mut input_zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = Cursor::new(Vec::new());
    let mut output_zip = zip::ZipWriter::new(&mut output);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for index in 0..input_zip.len() {
        let mut input_file = input_zip.by_index(index).unwrap();
        let name = if input_file.name().starts_with("metadata.") {
            metadata_name.to_owned()
        } else {
            input_file.name().to_owned()
        };

        output_zip.start_file(name, options).unwrap();
        std::io::copy(&mut input_file, &mut output_zip).unwrap();
    }
    output_zip.finish().unwrap();
    output.into_inner()
}

fn assert_single_journey_roundtrip(section_version: SectionVersion) {
    let (bytes, header, data) = write_single_journey_archive(section_version);
    assert_eq!(
        metadata_entry_name(&bytes),
        expected_metadata_name(section_version)
    );

    let expected_headers = vec![header.clone()];
    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.iter_journey_headers(), expected_headers.as_slice());
    assert_eq!(
        archive::for_testing::section_version_for_journey(&mut reader, &header.id).unwrap(),
        Some(section_version)
    );

    let loaded = reader.load_single_journey(&header.id).unwrap().unwrap();
    assert_eq!(loaded, (header, data));
}

#[test]
fn write_v1_and_read_back() {
    assert_single_journey_roundtrip(SectionVersion::V1);
}

#[test]
fn write_v2_and_read_back() {
    assert_single_journey_roundtrip(SectionVersion::V2);
}

#[test]
fn write_requested_metadata_name() {
    let (v1_bytes, _, _) = write_single_journey_archive(SectionVersion::V1);
    assert_eq!(metadata_entry_name(&v1_bytes), "metadata.xxm");

    let (v2_bytes, _, _) = write_single_journey_archive(SectionVersion::V2);
    assert_eq!(metadata_entry_name(&v2_bytes), "metadata.mldm");
}

#[test]
fn read_metadata_name_independent_of_section_version() {
    let (bytes, header, data) = write_single_journey_archive(SectionVersion::V1);
    let bytes = rewrite_metadata_entry_name(bytes, "metadata.mldm");

    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    let loaded = reader.load_single_journey(&header.id).unwrap().unwrap();
    assert_eq!(loaded, (header, data));
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
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, SectionVersion::V1))
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
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, SectionVersion::V1))
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
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, SectionVersion::V1))
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
