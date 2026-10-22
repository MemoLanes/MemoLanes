use std::{fs, io::Cursor};

use memolanes_core::{
    archive::{self, MldxReader},
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind, JourneyType},
    journey_metadata::{JourneyMetadata, JourneyMetadataKey},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::MainDb,
};
use protobuf::Message;
use tempdir::TempDir;

fn metadata() -> JourneyMetadata {
    let mut metadata = JourneyMetadata::default();
    metadata.set(JourneyMetadataKey::FlightNumber, "MU5101");
    metadata.set(JourneyMetadataKey::FlightAirline, "中国东方航空");
    // These deliberately have no corresponding enum variants.
    metadata.fields.insert(
        "future.operator_label".into(),
        "未来运营方 ✈️\n第二行".into(),
    );
    metadata.fields.insert("future.empty".into(), String::new());
    metadata
        .fields
        .insert("自定义.来源".into(), "用户补充".into());
    metadata
}

fn journey(metadata: Option<JourneyMetadata>) -> (JourneyHeader, JourneyData) {
    let start = "2026-10-22T01:00:00Z".parse().unwrap();
    let end = "2026-10-22T03:00:00Z".parse().unwrap();
    (
        JourneyHeader {
            id: "metadata-compatibility-flight".into(),
            revision: "metadata-compatibility-revision".into(),
            journey_date: "2026-10-22".parse().unwrap(),
            created_at: start,
            updated_at: None,
            start: Some(start),
            end: Some(end),
            journey_type: JourneyType::Vector,
            journey_kind: JourneyKind::Flight,
            note: Some("Metadata compatibility fixture".into()),
            postprocessor_algo: None,
            has_raw_data: false,
            metadata,
        },
        JourneyData::Vector(JourneyVector {
            track_segments: vec![TrackSegment {
                track_points: vec![
                    TrackPoint {
                        latitude: 31.2,
                        longitude: 121.3,
                    },
                    TrackPoint {
                        latitude: 40.1,
                        longitude: 116.6,
                    },
                ],
            }],
        }),
    )
}

fn encode(header: JourneyHeader, data: JourneyData) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    archive::export_single_journey_as_mldx(header, data, None, &mut output, false).unwrap();
    output.into_inner()
}

#[test]
fn optional_metadata_and_arbitrary_string_keys_round_trip_through_protobuf() {
    for metadata in [None, Some(JourneyMetadata::default()), Some(metadata())] {
        let (header, _) = journey(metadata);
        let bytes = header.clone().to_proto().write_to_bytes().unwrap();
        let decoded = Message::parse_from_bytes(&bytes).unwrap();
        assert_eq!(JourneyHeader::of_proto(decoded).unwrap(), header);
    }
}

#[test]
fn enum_accessors_modify_only_the_requested_key() {
    let mut metadata = metadata();
    let mut expected = metadata.fields.clone();
    metadata.set(JourneyMetadataKey::FlightNumber, "MU5102");
    assert_eq!(
        metadata.get(JourneyMetadataKey::FlightNumber),
        Some("MU5102")
    );
    assert_eq!(
        metadata.remove(JourneyMetadataKey::FlightNumber),
        Some("MU5102".into())
    );
    assert_eq!(metadata.get(JourneyMetadataKey::FlightNumber), None);
    expected.remove(JourneyMetadataKey::FlightNumber.as_str());
    assert_eq!(metadata.fields, expected);
}

#[test]
fn unknown_keys_survive_import_database_reopen_header_edit_and_export() {
    let (header, data) = journey(Some(metadata()));
    let bytes = encode(header.clone(), data.clone());
    let directory = TempDir::new("metadata-roundtrip").unwrap();
    {
        let mut db = MainDb::open(directory.path().to_str().unwrap()).unwrap();
        let mut reader = MldxReader::open(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.iter_journey_headers(), std::slice::from_ref(&header));
        let result = db.with_txn(|txn| reader.import(txn, None)).unwrap();
        assert_eq!(result.imported_count, 1);
    }
    let mut db = MainDb::open(directory.path().to_str().unwrap()).unwrap();
    db.with_txn(|txn| {
        txn.update_journey_info(
            &header.id,
            header.journey_date,
            header.start,
            header.end,
            Some("Edited note".into()),
            header.journey_kind,
        )
    })
    .unwrap();
    let mut output = Cursor::new(Vec::new());
    db.with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut output, false))
        .unwrap();
    let mut reader = MldxReader::open(Cursor::new(output.into_inner())).unwrap();
    let (exported, exported_data, raw) = reader
        .load_single_journey_with_raw_data(&header.id)
        .unwrap()
        .unwrap();
    assert_eq!(exported.metadata, header.metadata);
    assert_eq!(exported.note.as_deref(), Some("Edited note"));
    assert_ne!(exported.revision, header.revision);
    assert_eq!(exported_data, data);
    assert!(!exported.has_raw_data);
    assert!(raw.is_none());

    let destination = TempDir::new("metadata-reimport").unwrap();
    let mut db = MainDb::open(destination.path().to_str().unwrap()).unwrap();
    db.with_txn(|txn| reader.import(txn, None)).unwrap();
    let stored = db
        .with_txn(|txn| txn.get_journey_header(&header.id))
        .unwrap()
        .unwrap();
    assert_eq!(stored.metadata, header.metadata);
}

// The compatibility script requests files produced by the real new writer,
// then passes them to a separately compiled, pinned old core's importer.
#[test]
fn compatibility_fixtures_use_unchanged_archive_versions_and_track_layout() {
    let output_directory =
        std::env::var_os("MLDX_COMPAT_FIXTURE_DIR").map(std::path::PathBuf::from);
    if let Some(directory) = &output_directory {
        fs::create_dir_all(directory).unwrap();
    }
    for (name, metadata) in [
        ("absent", None),
        ("empty", Some(JourneyMetadata::default())),
        ("populated", Some(metadata())),
    ] {
        let (header, data) = journey(metadata);
        let bytes = encode(header.clone(), data.clone());
        let mut reader = MldxReader::open(Cursor::new(&bytes)).unwrap();
        assert_eq!(
            archive::for_testing::section_version_for_journey(&mut reader, &header.id).unwrap(),
            Some(2)
        );
        let loaded = reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, (header, data, None));
        if let Some(directory) = &output_directory {
            fs::write(directory.join(format!("{name}.mldx")), bytes).unwrap();
        }
    }
}
