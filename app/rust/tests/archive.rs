pub mod test_utils;

use anyhow::Ok;
use chrono::{DateTime, NaiveDate, Utc};
use memolanes_core::main_db::NewJourney;
use memolanes_core::{
    archive::{self, MldxReader},
    gps_processor, import_data,
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind, JourneyType},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::MainDb,
    raw_data::{self, ExtendedRawGPSPoint, JourneyRawData, JourneyRawDataHeader, RawGPSPoint},
};
use protobuf::Message;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs::File;
use std::io::Cursor;
use tempdir::TempDir;

fn add_vector_journeys(main_db: &mut MainDb) {
    let (raw_data, _preprocessor) =
        import_data::gpx::load_gpx("./tests/data/raw_gps_shanghai.gpx").unwrap();

    for (i, raw_data) in raw_data.iter().flatten().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            main_db
                .with_txn(|txn| txn.finalize_ongoing_journey(false))
                .unwrap();
        }
        main_db
            .record(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey(false))
        .unwrap();
}

fn add_bitmap_journey(main_db: &mut MainDb) {
    let (bitmap, _warnings) =
        import_data::fow::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    main_db
        .with_txn(|txn| {
            let _id = txn.create_and_insert_journey(NewJourney {
                journey_date: Utc::now().date_naive(),
                start: None,
                end: None,
                created_at: None,
                journey_kind: memolanes_core::journey_header::JourneyKind::DefaultKind,
                note: None,
                journey_data: JourneyData::Bitmap(bitmap),
                raw_data: None,
            })?;
            Ok(())
        })
        .unwrap()
}

fn all_journeys(main_db: &mut MainDb) -> Vec<(JourneyHeader, JourneyData)> {
    let journey_headers = main_db
        .with_txn(|txn| txn.query_journeys(None, None, None))
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
        has_raw_data: false,
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

fn sample_raw_data_with_latitude(latitude: f64) -> raw_data::SerializedJourneyRawData {
    JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
        points: vec![ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                point: gps_processor::Point {
                    latitude,
                    longitude: 121.4737,
                },
                timestamp_ms: Some(1_700_000_000_000),
                accuracy: Some(4.0),
                altitude: Some(12.0),
                speed: Some(1.0),
            },
            received_timestamp_ms: 1_700_000_000_010,
        }],
    }
    .serialize()
    .unwrap()
}

fn sample_raw_data() -> raw_data::SerializedJourneyRawData {
    sample_raw_data_with_latitude(31.2304)
}

// Frozen output of the former section-v1 writer, containing sample_journey().
const LEGACY_V1_ARCHIVE: &[u8] = include_bytes!("data/legacy_v1.mldx");

fn write_single_journey_archive() -> (Vec<u8>, JourneyHeader, JourneyData) {
    let (header, data) = sample_journey();
    let mut writer = Cursor::new(Vec::new());
    archive::export_single_journey_as_mldx(header.clone(), data.clone(), None, &mut writer, false)
        .unwrap();
    (writer.into_inner(), header, data)
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

fn section_entry_name(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    for index in 0..zip.len() {
        let name = zip.by_index(index).unwrap().name().to_owned();
        if !name.starts_with("metadata.") {
            return name;
        }
    }
    panic!("missing section entry")
}

fn write_single_journey_archive_with_raw_data(
    header: JourneyHeader,
    data: JourneyData,
    raw_data: Option<raw_data::SerializedJourneyRawData>,
    include_raw_data: bool,
) -> Vec<u8> {
    let mut writer = Cursor::new(Vec::new());
    archive::export_single_journey_as_mldx(header, data, raw_data, &mut writer, include_raw_data)
        .unwrap();
    writer.into_inner()
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

#[test]
fn read_and_import_legacy_v1() {
    let (header, data) = sample_journey();
    assert_eq!(metadata_entry_name(LEGACY_V1_ARCHIVE), "metadata.xxm");
    let mut reader = MldxReader::open(Cursor::new(LEGACY_V1_ARCHIVE)).unwrap();
    assert_eq!(reader.iter_journey_headers(), &[header.clone()]);
    assert_eq!(
        archive::for_testing::section_version_for_journey(&mut reader, &header.id).unwrap(),
        Some(1)
    );
    assert_eq!(
        reader.load_single_journey(&header.id).unwrap(),
        Some((header.clone(), data.clone()))
    );
    assert_eq!(
        reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap(),
        (header.clone(), data.clone(), None)
    );

    let temp_dir = TempDir::new("archive-import_legacy_v1").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    let result = main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(result.imported_count, 1);
    assert_raw_data(&mut main_db, &header.id, None);
    assert_eq!(all_journeys(&mut main_db), vec![(header, data)]);
}

#[test]
fn write_v2_and_read_back() {
    let (bytes, header, data) = write_single_journey_archive();
    assert_eq!(metadata_entry_name(&bytes), "metadata.mldm");
    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.iter_journey_headers(), &[header.clone()]);
    assert_eq!(
        archive::for_testing::section_version_for_journey(&mut reader, &header.id).unwrap(),
        Some(2)
    );
    assert_eq!(
        reader.load_single_journey(&header.id).unwrap(),
        Some((header.clone(), data.clone()))
    );
    assert_eq!(
        reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap(),
        (header, data, None)
    );
}

#[test]
fn v2_round_trips_raw_data() {
    let (mut header, data) = sample_journey();
    let raw_data = sample_raw_data();
    header.has_raw_data = true;
    let mut writer = Cursor::new(Vec::new());
    archive::export_single_journey_as_mldx(
        header.clone(),
        data.clone(),
        Some(raw_data.clone()),
        &mut writer,
        true,
    )
    .unwrap();

    let mut reader = MldxReader::open(Cursor::new(writer.into_inner())).unwrap();
    assert_eq!(
        reader.load_single_journey(&header.id).unwrap(),
        Some((header.clone(), data.clone()))
    );
    let loaded = reader
        .load_single_journey_with_raw_data(&header.id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded, (header, data, Some(raw_data)));
}

#[test]
fn extension_fields_preserve_following_records_on_read_and_import() {
    use integer_encoding::{VarIntReader, VarIntWriter};
    use std::io::{Read, Write};

    let source_dir = TempDir::new("archive-extension_source").unwrap();
    let mut source = MainDb::open(source_dir.path().to_str().unwrap()).unwrap();
    let (mut first, first_data) = sample_journey();
    first.id = "first-journey".to_owned();
    first.has_raw_data = true;
    let mut second = first.clone();
    second.id = "second-journey".to_owned();
    let second_data = JourneyData::Vector(JourneyVector {
        track_segments: vec![],
    });
    let expected = [
        (first, first_data, sample_raw_data()),
        (second, second_data, sample_raw_data_with_latitude(32.0)),
    ];
    let mut exported = Cursor::new(Vec::new());
    source
        .with_txn(|txn| {
            for (header, data, attachment) in &expected {
                txn.insert_journey_with_raw_data(
                    header.clone(),
                    data.clone(),
                    Some(attachment.clone()),
                )?;
            }
            archive::export_all_journeys_as_mldx(txn, &mut exported, true)
        })
        .unwrap();

    // Add an unknown field to the first record in a two-record section.
    // Rebuild the ZIP so entry sizes and checksums remain valid.
    let mut input = zip::ZipArchive::new(Cursor::new(exported.into_inner())).unwrap();
    assert_eq!(input.len(), 2, "Both journeys must share one section");
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        output.start_file(entry.name(), options).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if bytes.starts_with(b"MLS") {
            assert_eq!(bytes[3], 2);
            let mut section = Cursor::new(bytes.as_slice());
            section.set_position(4);
            let header_len: u64 = section.read_varint().unwrap();
            section.set_position(section.position() + header_len);
            let record_start = section.position() as usize;
            let field_count: u64 = section.read_varint().unwrap();
            assert_eq!(field_count, 2);
            let fields_start = section.position() as usize;
            for _ in 0..field_count {
                let len: u64 = section.read_varint().unwrap();
                section.set_position(section.position() + len);
            }
            let fields_end = section.position() as usize;
            assert!(fields_end < bytes.len(), "A second record must follow");
            output.write_all(&bytes[..record_start]).unwrap();
            output.write_varint(field_count + 1).unwrap();
            output.write_all(&bytes[fields_start..fields_end]).unwrap();
            let extension = b"future extension";
            output.write_varint(extension.len() as u64).unwrap();
            output.write_all(extension).unwrap();
            output.write_all(&bytes[fields_end..]).unwrap();
        } else {
            output.write_all(&bytes).unwrap();
        }
    }

    let bytes = output.finish().unwrap().into_inner();
    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    for (header, data, attachment) in &expected {
        assert_eq!(
            reader.load_single_journey(&header.id).unwrap(),
            Some((header.clone(), data.clone()))
        );
        assert_eq!(
            reader
                .load_single_journey_with_raw_data(&header.id)
                .unwrap(),
            Some((header.clone(), data.clone(), Some(attachment.clone())))
        );
    }

    let destination_dir = TempDir::new("archive-extension_destination").unwrap();
    let mut destination = MainDb::open(destination_dir.path().to_str().unwrap()).unwrap();
    let result = destination
        .with_txn(|txn| reader.import(txn, None))
        .unwrap();
    assert_eq!(result.imported_count, 2);
    for (header, data, attachment) in expected {
        destination
            .with_txn(|txn| {
                assert_eq!(txn.get_journey_header(&header.id)?, Some(header.clone()));
                assert_eq!(txn.get_journey_data(&header.id)?, data);
                assert_eq!(txn.get_journey_raw_data(&header.id)?, Some(attachment));
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn preview_does_not_read_raw_payload_but_full_reads_and_imports_validate_it() {
    use std::cell::Cell;
    use std::io::{Read, Seek, SeekFrom};
    use std::rc::Rc;

    struct CountingReader {
        inner: Cursor<Vec<u8>>,
        bytes_read: Rc<Cell<usize>>,
    }

    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let count = self.inner.read(buf)?;
            self.bytes_read.set(self.bytes_read.get() + count);
            std::result::Result::Ok(count)
        }
    }

    impl Seek for CountingReader {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(position)
        }
    }

    let (mut header, data) = sample_journey();
    header.has_raw_data = true;
    let payload_size = 1024 * 1024;
    // A large malformed attachment makes both accidental loading and decoding
    // observable. Preview only promises attachment presence, not validity.
    let attachment = raw_data::SerializedJourneyRawData::from_bytes(vec![0; payload_size]);
    let archive = write_single_journey_archive_with_raw_data(
        header.clone(),
        data.clone(),
        Some(attachment),
        true,
    );
    let bytes_read = Rc::new(Cell::new(0));
    let mut reader = MldxReader::open(CountingReader {
        inner: Cursor::new(archive),
        bytes_read: bytes_read.clone(),
    })
    .unwrap();
    bytes_read.set(0);
    assert_eq!(
        reader.load_single_journey(&header.id).unwrap(),
        Some((header.clone(), data))
    );
    assert!(bytes_read.get() < payload_size);
    assert!(reader
        .load_single_journey_with_raw_data(&header.id)
        .is_err());

    let temp_dir = TempDir::new("archive-invalid_raw_data_import").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    assert!(main_db.with_txn(|txn| reader.import(txn, None)).is_err());
    assert!(!main_db.with_txn(|txn| txn.has_journeys()).unwrap());
}

#[test]
fn export_preserves_mismatched_headers_and_import_repairs_them() {
    // Exercise both database and single-journey export with both mismatch kinds.
    for present in [false, true] {
        let (mut header, data) = sample_journey();
        header.has_raw_data = !present;
        let attachment = present.then(sample_raw_data);
        let mut corrected = header.clone();
        corrected.has_raw_data = present;
        if !present {
            corrected.revision.push('*');
        }

        let source_dir = TempDir::new("archive-mismatched_source").unwrap();
        let mut source = MainDb::open(source_dir.path().to_str().unwrap()).unwrap();
        source
            .with_txn(|txn| {
                txn.insert_journey_with_raw_data(header.clone(), data.clone(), attachment.clone())
            })
            .unwrap();
        // Normal writes reconcile attachment presence; inject a historical
        // inconsistency directly to test that export preserves actual content.
        let connection = Connection::open(source_dir.path().join("main.db")).unwrap();
        connection
            .execute(
                "UPDATE journey SET header = ?2 WHERE id = ?1;",
                (
                    &header.id,
                    header.clone().to_proto().write_to_bytes().unwrap(),
                ),
            )
            .unwrap();
        let mut bulk = Cursor::new(Vec::new());
        source
            .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut bulk, true))
            .unwrap();
        let single = write_single_journey_archive_with_raw_data(
            header.clone(),
            data.clone(),
            attachment.clone(),
            true,
        );

        for bytes in [bulk.into_inner(), single] {
            let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
            assert_eq!(reader.iter_journey_headers(), &[header.clone()]);
            assert_eq!(
                reader.load_single_journey(&header.id).unwrap(),
                Some((corrected.clone(), data.clone()))
            );
            assert_eq!(
                reader
                    .load_single_journey_with_raw_data(&header.id)
                    .unwrap(),
                Some((corrected.clone(), data.clone(), attachment.clone()))
            );

            let destination_dir = TempDir::new("archive-repaired_destination").unwrap();
            let mut destination = MainDb::open(destination_dir.path().to_str().unwrap()).unwrap();
            let result = destination
                .with_txn(|txn| reader.import(txn, None))
                .unwrap();
            assert_eq!(result.imported_count, 1);
            assert_eq!(
                all_journeys(&mut destination),
                vec![(corrected.clone(), data.clone())]
            );
            assert_raw_data(&mut destination, &header.id, attachment.as_ref());

            let repeated = destination
                .with_txn(|txn| reader.import(txn, None))
                .unwrap();
            assert_eq!(repeated.skipped_count, 1);
            assert_eq!(repeated.imported_count, 0);
            assert_eq!(repeated.overwritten_count, 0);
            assert_raw_data(&mut destination, &header.id, attachment.as_ref());
        }
        // Export must not repair or otherwise modify the source database.
        assert_eq!(
            source
                .with_txn(|txn| txn.get_journey_header(&header.id))
                .unwrap(),
            Some(header.clone())
        );
        assert_eq!(
            source
                .with_txn(|txn| txn.get_journey_raw_data(&header.id))
                .unwrap(),
            attachment
        );
    }
}

#[test]
fn import_compares_revisions_after_repairing_a_missing_attachment() {
    let (mut header, data) = sample_journey();
    header.has_raw_data = true;
    let bytes =
        write_single_journey_archive_with_raw_data(header.clone(), data.clone(), None, true);
    let directory = TempDir::new("archive-repaired_revision").unwrap();
    let mut destination = MainDb::open(directory.path().to_str().unwrap()).unwrap();
    destination
        .with_txn(|txn| {
            txn.insert_journey_with_raw_data(header.clone(), data, Some(sample_raw_data()))
        })
        .unwrap();

    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    let result = destination
        .with_txn(|txn| reader.import(txn, None))
        .unwrap();
    assert_eq!(result.imported_count, 1);
    assert_eq!(result.overwritten_count, 1);
    assert_eq!(result.skipped_count, 0);
    header.has_raw_data = false;
    header.revision.push('*');
    assert_eq!(
        destination
            .with_txn(|txn| txn.get_journey_header(&header.id))
            .unwrap(),
        Some(header.clone())
    );
    assert_raw_data(&mut destination, &header.id, None);
}

fn assert_raw_data(
    main_db: &mut MainDb,
    journey_id: &str,
    expected: Option<&raw_data::SerializedJourneyRawData>,
) {
    assert_eq!(
        main_db
            .with_txn(|txn| txn.get_journey_raw_data(journey_id))
            .unwrap()
            .as_ref(),
        expected
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.get_journey_header(journey_id))
            .unwrap()
            .unwrap()
            .has_raw_data,
        expected.is_some()
    );
}

#[test]
fn import_skips_same_revision_without_changing_raw_data() {
    let (header, data) = sample_journey();
    let raw_data = sample_raw_data();
    let temp_dir = TempDir::new("archive-import_skip_raw_data").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    main_db
        .with_txn(|txn| txn.insert_journey(header.clone(), data.clone()))
        .unwrap();

    let archive_with_raw_data = write_single_journey_archive_with_raw_data(
        header.clone(),
        data.clone(),
        Some(raw_data.clone()),
        true,
    );
    let mut reader = MldxReader::open(Cursor::new(archive_with_raw_data)).unwrap();
    let result = main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(result.skipped_count, 1);
    assert_raw_data(&mut main_db, &header.id, None);

    main_db
        .with_txn(|txn| {
            txn.delete_journey(&header.id)?;
            let mut header_with_raw_data = header.clone();
            header_with_raw_data.has_raw_data = true;
            txn.insert_journey_with_raw_data(
                header_with_raw_data,
                data.clone(),
                Some(raw_data.clone()),
            )
        })
        .unwrap();

    let mut reader = MldxReader::open(Cursor::new(LEGACY_V1_ARCHIVE)).unwrap();
    let result = main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(result.skipped_count, 1);
    assert_raw_data(&mut main_db, &header.id, Some(&raw_data));
    assert_eq!(
        main_db
            .with_txn(|txn| txn.get_journey_header(&header.id))
            .unwrap()
            .unwrap()
            .revision,
        header.revision
    );

    let omitted_v2 = write_single_journey_archive_with_raw_data(header.clone(), data, None, true);
    let mut reader = MldxReader::open(Cursor::new(omitted_v2)).unwrap();
    let result = main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(result.skipped_count, 1);
    assert_raw_data(&mut main_db, &header.id, Some(&raw_data));
}

#[test]
fn import_overwrites_raw_data_when_revision_differs() {
    let (mut local_header, data) = sample_journey();
    local_header.has_raw_data = true;
    let raw_data = sample_raw_data();
    let temp_dir = TempDir::new("archive-import_raw_data_overwrite").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    main_db
        .with_txn(|txn| {
            txn.insert_journey_with_raw_data(
                local_header.clone(),
                data.clone(),
                Some(raw_data.clone()),
            )
        })
        .unwrap();

    let mut edited_header = local_header.clone();
    edited_header.revision = "edited-revision".to_owned();
    let omitted_archive = write_single_journey_archive_with_raw_data(
        edited_header.clone(),
        data.clone(),
        Some(raw_data.clone()),
        false,
    );
    let mut reader = MldxReader::open(Cursor::new(omitted_archive)).unwrap();
    main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_raw_data(&mut main_db, &local_header.id, None);
    assert_eq!(
        main_db
            .with_txn(|txn| txn.get_journey_header(&local_header.id))
            .unwrap()
            .unwrap()
            .revision,
        "edited-revision*"
    );

    let replacement_raw_data = sample_raw_data_with_latitude(31.2305);
    let replacement_archive = write_single_journey_archive_with_raw_data(
        edited_header,
        data,
        Some(replacement_raw_data.clone()),
        true,
    );
    let mut reader = MldxReader::open(Cursor::new(replacement_archive)).unwrap();
    main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_raw_data(&mut main_db, &local_header.id, Some(&replacement_raw_data));
    assert_eq!(
        main_db
            .with_txn(|txn| txn.get_journey_header(&local_header.id))
            .unwrap()
            .unwrap()
            .revision,
        "edited-revision"
    );
}

#[test]
fn excluding_raw_data_from_export_appends_revision_marker() {
    let (mut header, data) = sample_journey();
    header.has_raw_data = true;
    let original_revision = header.revision.clone();
    let bytes = write_single_journey_archive_with_raw_data(
        header.clone(),
        data.clone(),
        Some(sample_raw_data()),
        false,
    );
    let section_name = section_entry_name(&bytes);

    let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
    let exported_header = reader.iter_journey_headers().first().unwrap();
    assert!(!exported_header.has_raw_data);
    assert_eq!(exported_header.revision, format!("{original_revision}*"));
    assert!(reader
        .load_single_journey_with_raw_data(&header.id)
        .unwrap()
        .unwrap()
        .2
        .is_none());

    let mut already_removed_header = header;
    already_removed_header.has_raw_data = false;
    already_removed_header.revision.push('*');
    let already_removed_bytes =
        write_single_journey_archive_with_raw_data(already_removed_header, data, None, false);
    assert_eq!(section_entry_name(&already_removed_bytes), section_name);
}

#[test]
fn raw_data_flags_are_stored_per_journey_in_the_same_section() {
    let temp_dir = TempDir::new("archive-per_journey_raw_data_flag").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    let (with_raw_data, data) = sample_journey();
    let mut without_raw_data = with_raw_data.clone();
    without_raw_data.id = "test-journey-without-raw-data".to_owned();
    without_raw_data.revision = "test-revision-without-raw-data".to_owned();

    main_db
        .with_txn(|txn| {
            txn.insert_journey_with_raw_data(
                with_raw_data.clone(),
                data.clone(),
                Some(sample_raw_data()),
            )?;
            txn.insert_journey(without_raw_data.clone(), data)
        })
        .unwrap();

    let mut archive = Cursor::new(Vec::new());
    main_db
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut archive, true))
        .unwrap();

    let reader = MldxReader::open(Cursor::new(archive.into_inner())).unwrap();
    let exported_with_raw_data = reader
        .iter_journey_headers()
        .iter()
        .find(|header| header.id == with_raw_data.id)
        .unwrap();
    let exported_without_raw_data = reader
        .iter_journey_headers()
        .iter()
        .find(|header| header.id == without_raw_data.id)
        .unwrap();
    assert!(exported_with_raw_data.has_raw_data);
    assert!(!exported_without_raw_data.has_raw_data);
}

#[test]
fn section_id_is_independent_of_export_option_without_raw_data() {
    let (header, data) = sample_journey();
    let first =
        write_single_journey_archive_with_raw_data(header.clone(), data.clone(), None, true);
    let second = write_single_journey_archive_with_raw_data(header, data, None, false);

    assert_eq!(section_entry_name(&first), section_entry_name(&second));
}

#[test]
fn updated_revision_changes_section_id() {
    let (mut header, data) = sample_journey();
    let raw_data = sample_raw_data();
    let first = write_single_journey_archive_with_raw_data(
        header.clone(),
        data.clone(),
        Some(raw_data.clone()),
        true,
    );
    header.revision = "updated-revision".to_owned();
    let second = write_single_journey_archive_with_raw_data(header, data, Some(raw_data), true);

    assert_ne!(section_entry_name(&first), section_entry_name(&second));
}

#[test]
fn read_metadata_name_independent_of_section_version() {
    let (v2_bytes, header, data) = write_single_journey_archive();
    for (bytes, metadata_name) in [
        (LEGACY_V1_ARCHIVE.to_vec(), "metadata.mldm"),
        (v2_bytes, "metadata.xxm"),
    ] {
        let bytes = rewrite_metadata_entry_name(bytes, metadata_name);
        let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
        let loaded = reader.load_single_journey(&header.id).unwrap().unwrap();
        assert_eq!(loaded, (header.clone(), data.clone()));
    }
}

#[test]
fn archive_and_import() {
    let temp_dir = TempDir::new("archive-archive_and_import").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);
    let mldx_file_path = temp_dir.path().join("archive.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    main_db
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, false))
        .unwrap();
    drop(file);
    main_db.with_txn(|txn| txn.delete_all_journeys()).unwrap();

    assert_eq!(
        metadata_entry_name(&std::fs::read(&mldx_file_path).unwrap()),
        "metadata.mldm"
    );
    let mut reader = MldxReader::open(File::open(&mldx_file_path).unwrap()).unwrap();
    for (header, _) in &all_journeys_before {
        assert_eq!(
            archive::for_testing::section_version_for_journey(&mut reader, &header.id).unwrap(),
            Some(2)
        );
    }
    main_db.with_txn(|txn| reader.import(txn, None)).unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn delete_all_journeys() {
    let temp_dir = TempDir::new("archive-delete_all_journeys").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();

    let all_journeys_before = all_journeys(&mut main_db);

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    main_db.with_txn(|txn| txn.delete_all_journeys()).unwrap();
    assert_eq!(all_journeys_before, all_journeys(&mut main_db));
}

#[test]
fn import_skips_existing_journeys() {
    let temp_dir = TempDir::new("archive-import_skips_existing_journeys").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();

    add_vector_journeys(&mut main_db);
    add_bitmap_journey(&mut main_db);

    let all_journeys_before = all_journeys(&mut main_db);
    let mldx_file_path = temp_dir.path().join("archive.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    main_db
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, false))
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
    let mut source_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
    add_vector_journeys(&mut source_db);
    add_bitmap_journey(&mut source_db);
    let all_from_archive = all_journeys(&mut source_db);

    let mldx_file_path = temp_dir.path().join("selected-import.mldx");
    let mut file = File::create(&mldx_file_path).unwrap();
    source_db
        .with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut file, false))
        .unwrap();
    drop(file);

    let target_dir = TempDir::new("archive-selected-target").unwrap();
    let mut target_db = MainDb::open(target_dir.path().to_str().unwrap()).unwrap();

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
