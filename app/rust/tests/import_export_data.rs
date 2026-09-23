#[macro_use]
extern crate assert_float_eq;

use chrono::{DateTime, NaiveDate, Utc};
use itertools::Itertools;
use memolanes_core::api::api::{
    export_all_journeys_as_gpx, export_all_journeys_as_kml, init, ExportResult,
};
use memolanes_core::api::import::{self as import_api, ImportPreprocessor, JourneyInfo};
use memolanes_core::export_data::gpx::raw_data_csv_to_gpx_file;
use memolanes_core::gpx_file_utils::{normalize_generic_time, normalize_step_of_my_world_time};
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::JourneyKind;
use memolanes_core::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use memolanes_core::main_db::MainDb;
use memolanes_core::{export_data, import_data};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;

fn run_gpx_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (parsed, preprocessor) = import_data::gpx::load_gpx(import_path).unwrap();
    let raw_data1 = parsed.flatten();
    let info = import_data::conversion::journey_info_from_raw_vector_data(&raw_data1);
    let vector1 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data1, None,
    )
    .unwrap();

    export_data::gpx::journey_vector_to_gpx_file(&vector1, &mut File::create(export_path).unwrap())
        .unwrap();

    let (parsed, _) = import_data::gpx::load_gpx(export_path).unwrap();
    let raw_data2 = parsed.flatten();
    let vector2 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data2, None,
    )
    .unwrap();

    let points1 = vector1
        .track_segments
        .into_iter()
        .flat_map(|t| t.track_points)
        .collect_vec();
    let points2 = vector2
        .track_segments
        .into_iter()
        .flat_map(|t| t.track_points)
        .collect_vec();

    assert_eq!(
        points1, points2,
        "Data integrity check failed for {import_path}"
    );
    (points1, info, preprocessor)
}

fn run_kml_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (parsed, preprocessor) = import_data::kml::load_kml(import_path).unwrap();
    let raw_data1 = parsed.flatten();
    let info = import_data::conversion::journey_info_from_raw_vector_data(&raw_data1);
    let vector1 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data1, None,
    )
    .unwrap();

    export_data::kml::journey_vector_to_kml_file(&vector1, &mut File::create(export_path).unwrap())
        .unwrap();

    let (parsed, _) = import_data::kml::load_kml(export_path).unwrap();
    let raw_data2 = parsed.flatten();
    let vector2 = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
        &raw_data2, None,
    )
    .unwrap();

    let points1 = vector1
        .track_segments
        .into_iter()
        .flat_map(|t| t.track_points)
        .collect_vec();
    let points2 = vector2
        .track_segments
        .into_iter()
        .flat_map(|t| t.track_points)
        .collect_vec();

    assert_eq!(
        points1, points2,
        "Data integrity check failed for {import_path}"
    );
    (points1, info, preprocessor)
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

    let (points, journey_info, preprocessor) = run_gpx_integrity_check(IMPORT_PATH, EXPORT_PATH);

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

    let (points, journey_info, preprocessor) = run_gpx_integrity_check(IMPORT_PATH, EXPORT_PATH);

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

    let (points, journey_info, preprocessor) = run_gpx_integrity_check(IMPORT_PATH, EXPORT_PATH);

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

    let (points, _journey_info, preprocessor) = run_gpx_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 53);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
pub fn kml_2bulu() {
    const IMPORT_PATH: &str = "./tests/data/2bulu.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/2bulu.kml";

    let (points, _journey_info, preprocessor) = run_kml_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 53);
    assert_f64_near!(points[0].latitude, 36.4375453);
    assert_f64_near!(points[0].longitude, 116.9442243);
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
}

#[test]
pub fn kml_track() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.kml";

    let (points, journey_info, preprocessor) = run_kml_integrity_check(IMPORT_PATH, EXPORT_PATH);

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

    let (points, _journey_info, _preprocessor) = run_kml_integrity_check(IMPORT_PATH, EXPORT_PATH);

    assert_eq!(points.len(), 10);
    assert_f64_near!(points[0].latitude, 36.6986802655);
    assert_f64_near!(points[0].longitude, 117.1179554744);
}

#[test]
fn test_raw_data_csv_to_gpx_file() {
    const CSV_PATH: &str = "./tests/data/raw_data.csv";
    const GPX_EXPORT_PATH: &str = "./tests/for_inspection/raw_data.gpx";

    let csv_file = File::open(CSV_PATH).unwrap();
    let mut reader = csv::Reader::from_reader(BufReader::new(csv_file));

    raw_data_csv_to_gpx_file(&mut reader, &mut File::create(GPX_EXPORT_PATH).unwrap()).unwrap();

    let gpx_file = File::open(GPX_EXPORT_PATH).unwrap();
    let gpx = gpx::read(&mut BufReader::new(gpx_file)).unwrap();

    assert_eq!(gpx.tracks.len(), 1);

    let track_points: Vec<_> = gpx.tracks[0]
        .segments
        .iter()
        .flat_map(|seg| seg.points.iter())
        .collect_vec();

    assert_eq!(track_points.len(), 929);

    assert_f64_near!(track_points[0].point().x(), -0.104277);
    assert_f64_near!(track_points[0].point().y(), 51.520302);

    let metadata = gpx.metadata.expect("GPX metadata should exist");
    assert_eq!(metadata.name.as_deref(), Some("MemoLanes RawData"));
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

#[test]
fn bulk_api_skips_unrepresentable_journeys_and_round_trips_metadata() {
    let temp = tempdir::TempDir::new("bulk-vector-export").unwrap();
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
    let gpx_path = temp.path().join("all.gpx");
    let kml_path = temp.path().join("all.kml");
    let export_gpx = || export_all_journeys_as_gpx(gpx_path.to_string_lossy().into_owned());
    let export_kml = || export_all_journeys_as_kml(kml_path.to_string_lossy().into_owned());

    assert!(matches!(export_gpx().unwrap(), ExportResult::DataIsEmpty));
    assert!(matches!(export_kml().unwrap(), ExportResult::DataIsEmpty));
    assert!(!gpx_path.exists());
    assert!(!kml_path.exists());

    let mut db = MainDb::open(&support).unwrap();
    let first_date = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
    let second_date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
    let start = DateTime::parse_from_rfc3339("2026-09-11T01:02:03Z")
        .unwrap()
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339("2026-09-11T02:03:04Z")
        .unwrap()
        .with_timezone(&Utc);
    db.with_txn(|txn| {
        txn.create_and_insert_journey(
            first_date,
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Bitmap(JourneyBitmap::new()),
        )?;
        txn.create_and_insert_journey(
            first_date,
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Vector(JourneyVector {
                track_segments: Vec::new(),
            }),
        )?;
        Ok(())
    })
    .unwrap();
    assert!(matches!(export_gpx().unwrap(), ExportResult::DataIsEmpty));
    assert!(matches!(export_kml().unwrap(), ExportResult::DataIsEmpty));

    let ids = db
        .with_txn(|txn| {
            let mut ids = Vec::new();
            for (date, offset) in [(first_date, 1.0), (first_date, 10.0), (second_date, 20.0)] {
                ids.push(txn.create_and_insert_journey(
                    date,
                    Some(start),
                    Some(end),
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    export_test_vector(offset),
                )?);
            }
            Ok(ids)
        })
        .unwrap();
    assert!(matches!(export_gpx().unwrap(), ExportResult::Succeed));
    assert!(matches!(export_kml().unwrap(), ExportResult::Succeed));
    assert!(fs::read_to_string(&gpx_path)
        .unwrap()
        .contains("xmlns:memolanes=\"https://app.memolanes.com/ns/journey/1\""));
    assert!(!fs::read_to_string(&kml_path)
        .unwrap()
        .contains("xmlns:memolanes"));

    let (gpx, _) = import_data::gpx::load_gpx(gpx_path.to_str().unwrap()).unwrap();
    let (kml, _) = import_data::kml::load_kml(kml_path.to_str().unwrap()).unwrap();
    for parsed in [gpx, kml] {
        assert_eq!(parsed.groups.len(), 3);
        let exported_ids = parsed
            .groups
            .iter()
            .map(|group| group.source_journey_id.as_deref().unwrap())
            .collect::<HashSet<_>>();
        assert_eq!(exported_ids, ids.iter().map(String::as_str).collect());
        for group in &parsed.groups {
            assert!(group.source_revision.is_some());
            assert_eq!(group.start, Some(start));
            assert_eq!(group.end, Some(end));
            let source_index = ids
                .iter()
                .position(|id| Some(id.as_str()) == group.source_journey_id.as_deref())
                .unwrap();
            let offset = [1.0, 10.0, 20.0][source_index];
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
                vec![
                    vec![(offset, 1.0, None), (offset + 1.0, 2.0, None)],
                    vec![(offset + 2.0, 3.0, None)],
                ]
            );
        }
        assert_eq!(
            parsed
                .groups
                .iter()
                .filter(|group| group.journey_date == Some(first_date))
                .count(),
            2
        );
        assert_eq!(
            parsed
                .groups
                .iter()
                .filter(|group| group.journey_date == Some(second_date))
                .count(),
            1
        );
    }
}
