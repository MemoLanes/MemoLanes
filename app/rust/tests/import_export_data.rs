use itertools::Itertools;
use memolanes_core::api::import::{ImportPreprocessor, JourneyInfo};
use memolanes_core::journey_vector::TrackPoint;
use memolanes_core::preclean::{normalize_generic_time, normalize_step_of_my_world_time};
use memolanes_core::{export_data, import_data};
use std::fs::File;

#[macro_use]
extern crate assert_float_eq;

fn run_gpx_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (raw_data1, preprocessor) = import_data::load_gpx(import_path).unwrap();
    let info = import_data::journey_info_from_raw_vector_data(&raw_data1);
    let vector1 = import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data1, false).unwrap();

    export_data::journey_vector_to_gpx_file(&vector1, &mut File::create(export_path).unwrap()).unwrap();

    let (raw_data2, _) = import_data::load_gpx(export_path).unwrap();
    let vector2 = import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data2, false).unwrap();

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

    assert_eq!(points1, points2, "Data integrity check failed for {}", import_path);
    (points1, info, preprocessor)
}

fn run_kml_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (raw_data1, preprocessor) = import_data::load_kml(import_path).unwrap();
    let info = import_data::journey_info_from_raw_vector_data(&raw_data1);
    let vector1 = import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data1, false).unwrap();

    export_data::journey_vector_to_kml_file(&vector1, &mut File::create(export_path).unwrap()).unwrap();

    let (raw_data2, _) = import_data::load_kml(export_path).unwrap();
    let vector2 = import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data2, false).unwrap();

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

    assert_eq!(points1, points2, "Data integrity check failed for {}", import_path);
    (points1, info, preprocessor)
}

#[test]
fn load_fow_sync_data() {
    let (bitmap_1, warnings_1) = import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let (bitmap_2, warnings_2) = import_data::load_fow_sync_data("./tests/data/fow_2.zip").unwrap();
    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{warnings_1:?}"), "None");
    assert_eq!(
        format!("{warnings_2:?}"),
        "Some(\"unexpected file: garbage_2\")"
    );
}

#[test]
fn verify_fow_snapshot_data() {
    let (bitmap_1, warnings_1) =
        import_data::load_fow_sync_data("./tests/data/snapshot_fow_test.zip").unwrap();
    let (bitmap_2, warnings_2) =
        import_data::load_fow_snapshot_data("./tests/data/snapshot_test.fwss").unwrap();
    let result_1 = import_data::load_fow_snapshot_data("./tests/data/snapshot_no_bitmap.fwss");

    assert!(
        !bitmap_2.tiles.is_empty(),
        "snapshot_test.fwss bitmap should not be empty"
    );
    assert!(result_1.is_err(), "Empty snapshot should return error");

    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{warnings_1:?}"), "None");
    assert_eq!(format!("{warnings_2:?}"), "None");
    assert_eq!(result_1.unwrap_err().to_string(), "empty data. warnings: ");
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
    assert!(matches!(preprocessor, ImportPreprocessor::Sparse));
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
    assert!(matches!(preprocessor, ImportPreprocessor::Sparse));
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
