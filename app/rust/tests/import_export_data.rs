#[macro_use]
extern crate assert_float_eq;

pub mod test_utils;

use chrono::{DateTime, NaiveDate, Utc};
use memolanes_core::api::api::{
    delete_journey_raw_data, export_all_journeys_as_gpx, export_all_journeys_as_kml,
    export_all_journeys_raw_data, export_journey, export_journey_raw_data, generate_full_archive,
    has_journeys_raw_data, init, ExportResult, ExportType,
};
use memolanes_core::api::import as import_api;
use memolanes_core::archive::MldxReader;
use memolanes_core::export_data::RawDataExportType;
use memolanes_core::gps::{ExtendedRawGPSPoint, Point, RawGPSPoint};
use memolanes_core::gpx_file_utils::{normalize_generic_time, normalize_step_of_my_world_time};
use memolanes_core::import_data::{ImportPreprocessor, ParsedVectorData};
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::{JourneyInfo, JourneyKind, JourneyType};
use memolanes_core::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use memolanes_core::main_db::{MainDb, NewJourney};
use memolanes_core::raw_data::{JourneyRawData, SerializedJourneyRawData};
use memolanes_core::{export_data, import_data};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::Path;

fn load_vector_fixture(path: &Path) -> (ParsedVectorData, ImportPreprocessor) {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("gpx") => import_data::gpx::load_gpx(path.to_str().unwrap()),
        Some("kml") => import_data::kml::load_kml(path.to_str().unwrap()),
        _ => panic!("Unsupported vector fixture: {}", path.display()),
    }
    .unwrap()
}

fn run_vector_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (parsed, preprocessor) = load_vector_fixture(Path::new(import_path));
    let raw_data1 = parsed.flatten();
    let info = import_data::conversion::journey_info_from_raw_vector_data(&raw_data1);
    let vector1 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data1, None,
    )
    .unwrap();

    let journeys = [test_utils::vector_export(vector1.clone())];
    let mut file = File::create(export_path).unwrap();
    match Path::new(export_path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("gpx") => export_data::gpx::journeys_to_gpx_file(&journeys, &mut file),
        Some("kml") => export_data::kml::journeys_to_kml_file(&journeys, &mut file),
        _ => panic!("Unsupported vector export: {export_path}"),
    }
    .unwrap();
    drop(file);

    let (parsed, _) = load_vector_fixture(Path::new(export_path));
    let raw_data2 = parsed.flatten();
    let vector2 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data2, None,
    )
    .unwrap();

    assert_eq!(
        vector1, vector2,
        "Data integrity check failed for {import_path}"
    );
    (
        vector1
            .track_segments
            .into_iter()
            .flat_map(|segment| segment.track_points)
            .collect(),
        info,
        preprocessor,
    )
}

#[test]
fn load_fow_sync_data() {
    let (bitmap_1, warnings_1) =
        import_data::fow::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let (bitmap_2, warnings_2) =
        import_data::fow::load_fow_sync_data("./tests/data/fow_2.zip").unwrap();
    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{warnings_1:?}"), "None");
    assert_eq!(
        format!("{warnings_2:?}"),
        "Some(\"unexpected file: garbage_2\")"
    );
}

#[test]
fn verify_fow_snapshot_data() {
    const SNAPSHOT_TEST_PATH: &str = "./tests/data/Snapshot-20260601T232045+0800.fwss";

    let (bitmap_1, warnings_1) =
        import_data::fow::load_fow_sync_data("./tests/data/snapshot_fow_test.zip").unwrap();
    let (bitmap_2, warnings_2) =
        import_data::fow::load_fow_snapshot_data(SNAPSHOT_TEST_PATH).unwrap();
    let result_1 = import_data::fow::load_fow_snapshot_data("./tests/data/snapshot_no_bitmap.fwss");
    let (journey_info, _journey_data) =
        import_api::load_fow_data(SNAPSHOT_TEST_PATH.to_owned()).unwrap();

    assert!(
        !bitmap_2.is_empty(),
        "Snapshot-20260601T232045+0800.fwss bitmap should not be empty"
    );
    assert!(result_1.is_err(), "Empty snapshot should return error");

    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{warnings_1:?}"), "None");
    assert_eq!(format!("{warnings_2:?}"), "None");
    assert_eq!(result_1.unwrap_err().to_string(), "empty data. warnings: ");
    assert_eq!(journey_info.journey_date.to_string(), "2026-06-01");
    assert_eq!(journey_info.start_time, None);
    assert_eq!(
        journey_info.end_time.map(|time| time.to_rfc3339()),
        Some("2026-06-01T15:20:45+00:00".to_owned())
    );
}

#[test]
fn parse_fwss_snapshot_time_from_filename_is_case_insensitive() {
    const SNAPSHOT_TEST_PATH: &str = "./tests/data/Snapshot-20260601T232045+0800.fwss";
    let temp_dir = tempdir::TempDir::new("mixed-case-fwss-snapshot").unwrap();
    let mixed_case_path = temp_dir.path().join("sNaPsHoT-20260601T232045+0800.fwss");
    std::fs::copy(SNAPSHOT_TEST_PATH, &mixed_case_path).unwrap();

    let (journey_info, _journey_data) =
        import_api::load_fow_data(mixed_case_path.to_string_lossy().into_owned()).unwrap();

    assert_eq!(journey_info.journey_date.to_string(), "2026-06-01");
    assert_eq!(
        journey_info.end_time.map(|time| time.to_rfc3339()),
        Some("2026-06-01T15:20:45+00:00".to_owned())
    );
}

#[test]
pub fn gpx() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.gpx";

    let (points, journey_info, preprocessor) = run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();

    assert_eq!(points.len(), 2945);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
fn test_normalize_times() {
    let input = "2025-07-02 18:07:33 +0000";
    let normalized = normalize_generic_time(input).unwrap();
    assert_eq!(normalized, "2025-07-02T18:07:33+00:00");

    let input = "2020-07-21T上午7:38:32Z";
    let normalized = normalize_step_of_my_world_time(input).unwrap();
    assert_eq!(normalized, "2020-07-21T07:38:32Z");

    let input_pm = "2020-07-21T下午7:38:32Z";
    let normalized_pm = normalize_step_of_my_world_time(input_pm).unwrap();
    assert_eq!(normalized_pm, "2020-07-21T19:38:32Z");
}

#[test]
pub fn gpx_step_of_my_world() {
    const IMPORT_PATH: &str = "./tests/data/StepOfMyWorld.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/StepOfMyWorld.gpx";

    let (points, journey_info, preprocessor) = run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();

    assert_eq!(points.len(), 313);
    assert_eq!(start_time, 1766592001000);
    assert_eq!(end_time, 1766678400000);
    assert!(matches!(preprocessor, ImportPreprocessor::Spare));
}

#[test]
pub fn gpx_your_app() {
    const IMPORT_PATH: &str = "./tests/data/yourapp.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/yourapp.gpx";

    let (points, journey_info, preprocessor) = run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();

    assert_eq!(points.len(), 400);
    assert_eq!(start_time, 1766624275000);
    assert_eq!(end_time, 1766665298000);
    assert!(matches!(preprocessor, ImportPreprocessor::Spare));
}

#[test]
pub fn gpx_2bulu() {
    const IMPORT_PATH: &str = "./tests/data/2bulu.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/2bulu.gpx";

    let (points, _journey_info, preprocessor) =
        run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 53);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
pub fn kml_2bulu() {
    const IMPORT_PATH: &str = "./tests/data/2bulu.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/2bulu.kml";

    let (points, _journey_info, preprocessor) =
        run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 53);
    assert_f64_near!(points[0].latitude, 36.4375453);
    assert_f64_near!(points[0].longitude, 116.9442243);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
pub fn kml_track() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.kml";

    let (points, journey_info, preprocessor) = run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    let start_time = journey_info
        .start_time
        .unwrap_or_default()
        .timestamp_millis();
    let end_time = journey_info.end_time.unwrap_or_default().timestamp_millis();

    assert_eq!(points.len(), 1651);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
pub fn kml_line_string() {
    const IMPORT_PATH: &str = "./tests/data/2024-08-24-2104.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/2024-08-24-2104.kml";

    let (points, _journey_info, _preprocessor) =
        run_vector_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 10);
    assert_f64_near!(points[0].latitude, 36.6986802655);
    assert_f64_near!(points[0].longitude, 117.1179554744);
}

#[test]
fn memolanes_gpx_preserves_tracks_routes_and_ignores_extensions() {
    let temp = tempdir::TempDir::new("mixed-memolanes-gpx").unwrap();
    let path = temp.path().join("mixed.gpx");
    fs::write(
        &path,
        r#"<gpx version="1.1" creator="MemoLanes" xmlns="http://www.topografix.com/GPX/1/1"
            xmlns:ml="https://app.memolanes.com/ns/journey/1">
          <rte><rtept lat="3" lon="4"/><rtept lat="3.1" lon="4.1"/></rte>
          <trk><type>MemoLanes Journey</type>
            <extensions><ml:journey date="2026-09-11"/></extensions>
            <trkseg><trkpt lat="1" lon="2"/><trkpt lat="1.1" lon="2.1"/></trkseg>
          </trk>
        </gpx>"#,
    )
    .unwrap();
    for (path, exported) in [
        (path.as_path(), false),
        (
            Path::new("./tests/data/memolanes_foreign_extensions.gpx"),
            true,
        ),
    ] {
        let label = path.display();
        let (parsed, preprocessor) = load_vector_fixture(path);
        assert_eq!(parsed.is_exported_journey(), exported, "{label}");
        assert!(matches!(preprocessor, ImportPreprocessor::None), "{label}");
        if exported {
            assert_eq!(parsed.groups().len(), 1);
            assert_eq!(
                parsed.groups()[0].journey_date,
                NaiveDate::from_ymd_opt(2026, 9, 11)
            );
        }
        let segments = parsed.flatten();
        assert_eq!(
            segments.iter().map(Vec::len).collect::<Vec<_>>(),
            [2, 2],
            "{label}"
        );
        assert_eq!(
            segments
                .iter()
                .flatten()
                .map(|point| (point.point.latitude, point.point.longitude))
                .collect::<Vec<_>>(),
            [(1.0, 2.0), (1.1, 2.1), (3.0, 4.0), (3.1, 4.1)],
            "{label}",
        );
    }
}

#[test]
fn mixed_memolanes_kml_preserves_geometry_without_duplicates() {
    let (parsed, preprocessor) =
        import_data::kml::load_kml("./tests/data/mixed_memolanes_geometry.kml").unwrap();
    assert!(!parsed.is_exported_journey());
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
    let segments = parsed.flatten();
    assert_eq!(segments.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2]);
    for (segment, latitude) in segments.iter().zip([1.0, 3.0]) {
        assert_eq!(segment[0].point.latitude, latitude);
        assert_eq!(segment[1].point.latitude, latitude + 0.1);
    }
    let timestamp = |time| {
        DateTime::parse_from_rfc3339(time)
            .unwrap()
            .timestamp_millis()
    };
    assert_eq!(segments[0][0].timestamp_ms, None);
    assert_eq!(
        segments[1][0].timestamp_ms,
        Some(timestamp("2026-09-11T01:00:00Z"))
    );
}

fn export_test_vector(offset: f64) -> JourneyData {
    JourneyData::Vector(JourneyVector {
        track_segments: vec![
            TrackSegment {
                track_points: vec![
                    TrackPoint {
                        latitude: offset,
                        longitude: 1.0,
                    },
                    TrackPoint {
                        latitude: offset + 1.0,
                        longitude: 2.0,
                    },
                ],
            },
            TrackSegment {
                track_points: vec![TrackPoint {
                    latitude: offset + 2.0,
                    longitude: 3.0,
                }],
            },
        ],
    })
}

fn export_test_raw_data(latitude: f64) -> JourneyRawData {
    let points = [Some(1_700_000_000_000), None]
        .into_iter()
        .enumerate()
        .map(|(index, timestamp_ms)| ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                point: Point {
                    latitude: latitude + index as f64 * 0.001,
                    longitude: 121.473701,
                },
                timestamp_ms,
                accuracy: None,
                altitude: None,
                speed: None,
            },
            received_timestamp_ms: 1_700_000_001_234 + index as i64,
        })
        .collect();
    JourneyRawData::new(points, 1_700_000_000_000)
}

#[test]
fn bulk_export_import_preserves_journeys_and_raw_data() {
    let temp = tempdir::TempDir::new("bulk-export-import").unwrap();
    let subdir = |name: &str| {
        let path = temp.path().join(name);
        fs::create_dir(&path).unwrap();
        path.to_string_lossy().into_owned()
    };
    let support = subdir("support");
    init(
        subdir("temp"),
        subdir("doc"),
        support.clone(),
        subdir("cache"),
    )
    .unwrap();
    let mut db = MainDb::open(&support).unwrap();
    let gpx_path = temp.path().join("all.gpx");
    let kml_path = temp.path().join("all.kml");
    let export_gpx = || export_all_journeys_as_gpx(gpx_path.to_string_lossy().into_owned());
    let export_kml = || export_all_journeys_as_kml(kml_path.to_string_lossy().into_owned());
    let assert_empty_vectors = || {
        for (result, path) in [(export_gpx(), &gpx_path), (export_kml(), &kml_path)] {
            assert!(matches!(result.unwrap(), ExportResult::DataIsEmpty));
            assert!(!path.exists());
        }
    };
    assert_empty_vectors();

    let first_date = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
    let second_date = first_date.succ_opt().unwrap();
    let start = DateTime::parse_from_rfc3339("2026-09-11T01:02:03Z")
        .unwrap()
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339("2026-09-11T02:03:04Z")
        .unwrap()
        .with_timezone(&Utc);
    db.with_txn(|txn| {
        for journey_data in [
            JourneyData::Bitmap(JourneyBitmap::new()),
            JourneyData::Vector(JourneyVector {
                track_segments: Vec::new(),
            }),
        ] {
            txn.create_and_insert_journey(NewJourney {
                journey_date: first_date,
                start: None,
                end: None,
                created_at: None,
                journey_kind: JourneyKind::DefaultKind,
                note: None,
                journey_data,
                raw_data: None,
            })?;
        }
        Ok(())
    })
    .unwrap();
    assert_empty_vectors();

    let ids = db
        .with_txn(|txn| {
            [(first_date, 1.0), (first_date, 10.0), (second_date, 20.0)]
                .into_iter()
                .map(|(journey_date, offset)| {
                    txn.create_and_insert_journey(NewJourney {
                        journey_date,
                        start: Some(start),
                        end: Some(end),
                        created_at: None,
                        journey_kind: JourneyKind::DefaultKind,
                        note: None,
                        journey_data: export_test_vector(offset),
                        raw_data: None,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .unwrap();
    // Read back real MLDX exports to cover buffered writes and seeking.
    let mldx_path = temp.path().join("journeys.mldx");
    for all_journeys in [true, false] {
        let result = if all_journeys {
            generate_full_archive(mldx_path.to_string_lossy().into_owned(), false)
        } else {
            export_journey(
                mldx_path.to_string_lossy().into_owned(),
                ids[0].clone(),
                ExportType::MLDX,
                false,
            )
        };
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        let mut archive = MldxReader::open(File::open(&mldx_path).unwrap()).unwrap();
        for id in if all_journeys { &ids[..] } else { &ids[..1] } {
            let expected = db
                .with_txn(|txn| {
                    Ok((
                        txn.get_journey_header(id)?.unwrap(),
                        txn.get_journey_data(id)?,
                    ))
                })
                .unwrap();
            assert_eq!(archive.load_single_journey(id).unwrap(), Some(expected));
        }
    }
    let parse_vector = |path: &Path| load_vector_fixture(path).0;
    let legacy_revision = db
        .with_txn(|txn| Ok(txn.get_journey_header(&ids[1])?.unwrap().revision))
        .unwrap();
    let assert_no_identity = |xml: &str| {
        for field in [
            "sourceJourneyId",
            "sourceRevision",
            "version=\"1\"",
            "memolanes:version",
        ] {
            assert!(!xml.contains(field));
        }
        for identity in ids.iter().chain([&legacy_revision]) {
            assert!(!xml.contains(identity));
        }
    };

    // Both formats preserve separate journeys, metadata and multi-segment geometry.
    for (format, path, result) in [
        (ExportType::GPX, &gpx_path, export_gpx()),
        (ExportType::KML, &kml_path, export_kml()),
    ] {
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        let xml = fs::read_to_string(path).unwrap();
        assert_no_identity(&xml);
        let parsed = parse_vector(path);
        let mut offsets = Vec::new();
        for group in parsed.groups() {
            assert!(group.has_memolanes_metadata());
            assert_eq!(group.start, Some(start));
            assert_eq!(group.end, Some(end));
            let offset = group.segments[0][0].point.latitude;
            offsets.push(offset as i32);
            assert_eq!(
                group.journey_date,
                Some(if offset == 20.0 {
                    second_date
                } else {
                    first_date
                })
            );
            let points = group
                .segments
                .iter()
                .map(|segment| {
                    segment
                        .iter()
                        .map(|point| {
                            (
                                point.point.latitude,
                                point.point.longitude,
                                point.timestamp_ms,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                points,
                [
                    vec![(offset, 1.0, None), (offset + 1.0, 2.0, None)],
                    vec![(offset + 2.0, 3.0, None)],
                ]
            );
        }
        offsets.sort_unstable();
        assert_eq!(offsets, [1, 10, 20]);

        let extension = path.extension().unwrap().to_str().unwrap();
        let invalid_metadata = match format {
            ExportType::GPX => {
                xml.replacen(&format!("date=\"{first_date}\""), "date=\"invalid\"", 1)
            }
            ExportType::KML => xml.replacen(
                &format!("<Data name=\"memolanes:date\"><value>{first_date}</value></Data>"),
                "<Data name=\"memolanes:date\"><value>invalid</value></Data>",
                1,
            ),
            _ => unreachable!(),
        };
        let fallback_path = temp.path().join(format!("invalid-metadata.{extension}"));
        fs::write(&fallback_path, invalid_metadata).unwrap();
        let fallback = parse_vector(&fallback_path);
        assert!(!fallback.is_exported_journey());
        assert!(fallback
            .groups()
            .iter()
            .all(|group| group.journey_date.is_none()
                && group.start.is_none()
                && group.end.is_none()));
        assert_eq!(fallback.flatten(), parsed.flatten());

        let single = temp.path().join(format!("single.{extension}"));
        assert!(matches!(
            export_journey(
                single.to_string_lossy().into_owned(),
                ids[0].clone(),
                format,
                false
            )
            .unwrap(),
            ExportResult::Succeed
        ));
        assert_no_identity(&fs::read_to_string(&single).unwrap());
        let single = parse_vector(&single);
        assert_eq!(single.groups().len(), 1);
        assert!(parsed.groups().contains(&single.groups()[0]));

        // Identity fields remain ignored, including invalid values and matching local IDs.
        for (name, source_id, revision, version) in [
            ("legacy", ids[1].as_str(), legacy_revision.as_str(), "1"),
            ("ignored", "&unknown;", "&unknown;", "unknown"),
        ] {
            let variant = match format {
                ExportType::GPX => xml.replace(
                    "<memolanes:journey ",
                    &format!("<memolanes:journey version=\"{version}\" sourceJourneyId=\"{source_id}\" sourceId=\"legacy\" sourceRevision=\"{revision}\" "),
                ),
                ExportType::KML => xml.replace(
                    "<ExtendedData>",
                    &format!("<ExtendedData><Data name=\"memolanes:version\"><value>{version}</value></Data><Data name=\"memolanes:sourceJourneyId\"><value>{source_id}</value></Data><Data name=\"memolanes:sourceRevision\"><value>{revision}</value></Data>"),
                ),
                _ => unreachable!(),
            };
            let variant_path = temp.path().join(format!("{name}.{extension}"));
            fs::write(&variant_path, variant).unwrap();
            assert_eq!(parse_vector(&variant_path), parsed);
        }

        // Equivalent XML encodings must preserve the same journey boundaries and metadata.
        let variants = match format {
            ExportType::GPX => {
                let namespace = "https://app.memolanes.com/ns/journey/1";
                let declaration = format!(" xmlns:memolanes=\"{namespace}\"");
                let local = xml.replace(&declaration, "");
                let rebound = temp.path().join("rebound.gpx");
                fs::write(
                    &rebound,
                    xml.replace(
                        "<trk>",
                        "<trk xmlns:memolanes=\"https://example.com/unrelated\">",
                    ),
                )
                .unwrap();
                let rebound = parse_vector(&rebound);
                assert!(rebound
                    .groups()
                    .iter()
                    .all(|group| !group.has_memolanes_metadata()));
                assert_eq!(rebound.flatten(), parsed.flatten());
                vec![
                    (
                        "extensions",
                        local.replace("<extensions>", &format!("<extensions{declaration}>")),
                    ),
                    (
                        "journey",
                        local.replace(
                            "<memolanes:journey ",
                            &format!("<memolanes:journey{declaration} "),
                        ),
                    ),
                    (
                        "default_namespace",
                        local.replace(
                            "<memolanes:journey ",
                            &format!("<journey xmlns=\"{namespace}\" "),
                        ),
                    ),
                ]
            }
            ExportType::KML => {
                let cdata =
                    ["coordinates", "value", "when"]
                        .into_iter()
                        .fold(xml.clone(), |xml, tag| {
                            xml.replace(&format!("<{tag}>"), &format!("<{tag}><![CDATA["))
                                .replace(&format!("</{tag}>"), &format!("]]></{tag}>"))
                        });
                let mixed = cdata
                    .replace("<![CDATA[2026-", "2<!--split-->&#48;<![CDATA[26-")
                    .replace(',', "]]>&#44;<!--split--><![CDATA[")
                    .replace('\n', "]]> \n<![CDATA[");
                let timestamp = [first_date, second_date].into_iter().fold(cdata.clone(), |xml, date| {
                    xml.replace(&format!("<Data name=\"memolanes:date\"><value><![CDATA[{date}]]></value></Data>"), "")
                });
                vec![
                    ("cdata", cdata),
                    ("mixed_text", mixed),
                    ("timestamp", timestamp),
                ]
            }
            _ => unreachable!(),
        };
        for (name, variant) in variants {
            let variant_path = temp.path().join(format!("{name}.{extension}"));
            fs::write(&variant_path, variant).unwrap();
            assert_eq!(parse_vector(&variant_path), parsed, "{name}.{extension}");
        }
    }

    let import_file = |path: &std::path::Path| {
        let (_, raw, preprocessor) =
            import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
        assert!(matches!(preprocessor, ImportPreprocessor::None));
        let parts = import_api::analyze_vector_data_parts(&raw);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.is_memolanes_journey));
        assert_eq!(
            parts
                .iter()
                .filter(|part| part.journey_date == first_date.to_string())
                .count(),
            2
        );
        assert_eq!(
            parts
                .iter()
                .map(|part| part.part_key.as_str())
                .collect::<HashSet<_>>()
                .len(),
            3
        );
        import_api::import_vector_data_by_parts(
            &raw,
            parts.iter().map(|part| part.part_key.clone()).collect(),
            // An explicit Generic request must still preserve exported segments.
            ImportPreprocessor::Generic,
            JourneyKind::DefaultKind,
            None,
        )
        .unwrap()
    };

    // Reimports skip matching geometry without overwriting local edits.
    assert_eq!(import_file(&gpx_path), 0);
    db.with_txn(|txn| {
        txn.update_journey_metadata(
            &ids[0],
            first_date,
            Some(start),
            Some(end),
            Some("local edit".to_owned()),
            JourneyKind::DefaultKind,
        )
    })
    .unwrap();
    assert_eq!(import_file(&gpx_path), 0);
    db.with_txn(|txn| {
        txn.update_journey_data_with_latest_postprocessor(&ids[0], export_test_vector(30.0))
    })
    .unwrap();
    let local_revision = db
        .with_txn(|txn| Ok(txn.get_journey_header(&ids[0])?.unwrap().revision))
        .unwrap();
    assert_eq!(import_file(&temp.path().join("legacy.gpx")), 1);
    assert_eq!(import_file(&gpx_path), 0);
    assert_eq!(import_file(&temp.path().join("legacy.kml")), 0);
    db.with_txn(|txn| {
        txn.update_journey_data_with_latest_postprocessor(&ids[1], export_test_vector(40.0))
    })
    .unwrap();
    assert_eq!(import_file(&temp.path().join("legacy.kml")), 1);
    assert_eq!(import_file(&gpx_path), 0);

    db.with_txn(|txn| {
        let local = txn.get_journey_header(&ids[0])?.unwrap();
        assert_eq!(local.revision, local_revision);
        assert_eq!(local.note.as_deref(), Some("local edit"));
        for header in txn.query_journeys(None, None, None)? {
            if header.journey_type == JourneyType::Vector {
                txn.delete_journey(&header.id)?;
            }
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(import_file(&gpx_path), 3);
    assert_eq!(import_file(&gpx_path), 0);
    assert_eq!(import_file(&kml_path), 0);
    db.with_txn(|txn| {
        let headers = txn
            .query_journeys(None, None, None)?
            .into_iter()
            .filter(|header| header.journey_type == JourneyType::Vector)
            .collect::<Vec<_>>();
        assert_eq!(headers.len(), 3);
        let mut offsets = Vec::new();
        for header in headers {
            assert_eq!(header.start, Some(start));
            assert_eq!(header.end, Some(end));
            let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
                panic!("imported journey is not vector data");
            };
            let offset = vector.track_segments[0].track_points[0].latitude;
            assert_eq!(
                header.journey_date,
                if offset == 20.0 {
                    second_date
                } else {
                    first_date
                }
            );
            assert_eq!(JourneyData::Vector(vector), export_test_vector(offset));
            offsets.push(offset as i32);
        }
        offsets.sort_unstable();
        assert_eq!(offsets, [1, 10, 20]);
        Ok(())
    })
    .unwrap();

    // Raw exports include only non-empty v2 attachments, even without vector data.
    let raw_formats = [
        (RawDataExportType::CSV, "csv"),
        (RawDataExportType::GPX, "gpx"),
        (RawDataExportType::KML, "kml"),
    ];
    let export_raw = |format, extension: &str| {
        let path = temp.path().join(format!("raw.{extension}"));
        (
            export_all_journeys_raw_data(path.to_string_lossy().into_owned(), format),
            path,
        )
    };
    let assert_empty_raw = || {
        assert!(!has_journeys_raw_data().unwrap());
        for (format, extension) in raw_formats {
            let (result, path) = export_raw(format, extension);
            assert!(matches!(result.unwrap(), ExportResult::DataIsEmpty));
            assert!(!path.exists());
        }
    };
    assert_empty_raw();
    fs::create_dir(format!("{support}/raw_data")).unwrap();
    fs::copy(
        "./tests/data/raw_data.csv",
        format!("{support}/raw_data/2024-01.csv"),
    )
    .unwrap();
    let insert_raw = |db: &mut MainDb, date, raw_data: SerializedJourneyRawData| {
        db.with_txn(|txn| {
            txn.create_and_insert_journey(NewJourney {
                journey_date: date,
                start: None,
                end: None,
                created_at: None,
                journey_kind: JourneyKind::DefaultKind,
                note: None,
                journey_data: JourneyData::Bitmap(JourneyBitmap::new()),
                raw_data: Some(raw_data),
            })
        })
        .unwrap()
    };
    insert_raw(
        &mut db,
        first_date,
        JourneyRawData::new(Vec::new(), 0).serialize().unwrap(),
    );
    assert_empty_raw();

    let latitudes = [31.230416, 32.230416];
    let ids = latitudes
        .iter()
        .map(|latitude| {
            insert_raw(
                &mut db,
                first_date,
                export_test_raw_data(*latitude).serialize().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert!(has_journeys_raw_data().unwrap());
    for (format, extension) in raw_formats {
        let (result, path) = export_raw(format, extension);
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        let segments = if format == RawDataExportType::CSV {
            let mut reader = csv::Reader::from_path(&path).unwrap();
            let headers = reader.headers().unwrap();
            assert_eq!(headers.len(), 8);
            assert_eq!(&headers[7], "journey_id");
            let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
            assert_eq!(rows.len(), 4);
            for (id, latitude) in ids.iter().zip(latitudes) {
                let rows = rows
                    .iter()
                    .filter(|row| &row[7] == id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][2].parse::<f64>().unwrap(), latitude);
            }
            let (_, raw, preprocessor) =
                import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
            let parts = import_api::analyze_vector_data_parts(&raw);
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].journey_date, parts[1].journey_date);
            assert!(parts
                .iter()
                .all(|part| part.is_explicit_journey && !part.is_memolanes_journey));
            let import_raw = |note| {
                import_api::import_vector_data_by_parts(
                    &raw,
                    parts.iter().map(|part| part.part_key.clone()).collect(),
                    preprocessor,
                    JourneyKind::DefaultKind,
                    note,
                )
                .unwrap()
            };
            assert_eq!(import_raw(Some("raw CSV import".to_owned())), 2);
            db.with_txn(|txn| {
                let imported = txn
                    .query_journeys(None, None, None)?
                    .into_iter()
                    .filter(|header| header.note.as_deref() == Some("raw CSV import"))
                    .collect::<Vec<_>>();
                assert_eq!(imported.len(), 2);
                let mut imported_latitudes = Vec::new();
                for header in imported {
                    assert!(!header.has_raw_data);
                    let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
                        panic!("CSV import is not vector data");
                    };
                    assert_eq!(vector.track_segments.len(), 1);
                    assert_eq!(vector.track_segments[0].track_points.len(), 2);
                    imported_latitudes.push(vector.track_segments[0].track_points[0].latitude);
                }
                imported_latitudes.sort_by(f64::total_cmp);
                assert_eq!(imported_latitudes, latitudes);
                Ok(())
            })
            .unwrap();

            // IDs select source groups; CSV reimports still create new journeys.
            assert_eq!(import_raw(None), 2);
            import_data::csv::load_csv(path.to_str().unwrap())
                .unwrap()
                .0
                .flatten()
        } else {
            let text = fs::read_to_string(&path).unwrap();
            for id in &ids {
                assert!(!text.contains(id));
            }
            for index in 1..=2 {
                assert!(text.contains(&format!("MemoLanes Raw Data {first_date} #{index}")));
            }
            let parsed = parse_vector(&path);
            assert!(!parsed.is_exported_journey());
            parsed.flatten()
        };
        assert_eq!(segments.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2]);
        let mut exported_latitudes = segments
            .iter()
            .map(|segment| segment[0].point.latitude)
            .collect::<Vec<_>>();
        exported_latitudes.sort_by(f64::total_cmp);
        assert_eq!(exported_latitudes, latitudes);
    }

    // Fail after writing valid journeys, remove partial files, then refresh availability.
    let corrupt_id = insert_raw(
        &mut db,
        first_date.pred_opt().unwrap(),
        SerializedJourneyRawData::from_bytes(b"invalid".to_vec()),
    );
    for (format, extension) in raw_formats {
        let (result, path) = export_raw(format, extension);
        assert!(result.is_err());
        assert!(!path.exists());
    }
    // Single exports must also remove files when a writer rejects a GPS timestamp.
    let mut invalid_raw = export_test_raw_data(31.0);
    invalid_raw.points[1].raw_gps_point.timestamp_ms = Some(i64::MAX);
    let invalid_id = insert_raw(&mut db, first_date, invalid_raw.serialize().unwrap());
    for (format, extension) in &raw_formats[1..] {
        let path = temp.path().join(format!("single-raw.{extension}"));
        assert!(export_journey_raw_data(
            path.to_string_lossy().into_owned(),
            invalid_id.clone(),
            *format,
        )
        .is_err());
        assert!(!path.exists());
    }
    for id in ids.into_iter().chain([corrupt_id, invalid_id]) {
        delete_journey_raw_data(id).unwrap();
    }
    assert_empty_raw();
}
