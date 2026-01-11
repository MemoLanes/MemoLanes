use itertools::Itertools;
use memolanes_core::{export_data, import_data};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use memolanes_core::api::api::export_raw_data_gpx_file;
use memolanes_core::export_data::raw_data_csv_to_gpx_file;
use memolanes_core::gps_processor::RawData;

#[macro_use]
extern crate assert_float_eq;

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
    let raw_vecotr_data1 = import_data::load_gpx(IMPORT_PATH).unwrap();
    let journey_info = import_data::journey_info_from_raw_vector_data(&raw_vecotr_data1);
    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();
    let vector1 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data1, false)
            .unwrap();
    export_data::journey_vector_to_gpx_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();

    let raw_vecotr_data2 = import_data::load_gpx(EXPORT_PATH).unwrap();
    let vector2 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data2, false)
            .unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();

    assert_eq!(points1.len(), 2945);
    assert_eq!(points1, points2);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
}

#[test]
pub fn gpx_2bulu() {
    const IMPORT_PATH: &str = "./tests/data/2bulu.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/2bulu.gpx";
    let raw_vecotr_data1 = import_data::load_gpx(IMPORT_PATH).unwrap();
    import_data::journey_info_from_raw_vector_data(&raw_vecotr_data1);
    let vector1 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data1, false)
            .unwrap();
    export_data::journey_vector_to_gpx_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();

    let raw_vecotr_data2 = import_data::load_gpx(EXPORT_PATH).unwrap();
    let vector2 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data2, false)
            .unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();

    assert_eq!(points1.len(), 53);
    assert_eq!(points1, points2);
}

#[test]
pub fn kml_2bulu() {
    const IMPORT_PATH: &str = "./tests/data/2bulu.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/2bulu.kml";
    let raw_vecotr_data1 = import_data::load_kml(IMPORT_PATH).unwrap();
    let vector1 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data1, false)
            .unwrap();

    export_data::journey_vector_to_kml_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();
    let raw_vecotr_data2 = import_data::load_kml(EXPORT_PATH).unwrap();
    let vector2 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data2, false)
            .unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();

    assert_eq!(points1.len(), 53);
    assert_eq!(points1, points2);
    assert_f64_near!(points1[0].latitude, 36.4375453);
    assert_f64_near!(points1[0].longitude, 116.9442243);
}

#[test]
pub fn kml_track() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.kml";
    let raw_vecotr_data1 = import_data::load_kml(IMPORT_PATH).unwrap();
    let journey_info = import_data::journey_info_from_raw_vector_data(&raw_vecotr_data1);
    let start_time = journey_info
        .start_time
        .unwrap_or_default()
        .timestamp_millis();
    let end_time = journey_info.end_time.unwrap_or_default().timestamp_millis();
    let vector1 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data1, false)
            .unwrap();

    export_data::journey_vector_to_kml_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();
    let raw_vecotr_data2 = import_data::load_kml(EXPORT_PATH).unwrap();
    let vector2 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data2, false)
            .unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();

    assert_eq!(points1.len(), 1651);
    assert_eq!(points1, points2);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
}

#[test]
pub fn kml_line_string() {
    const IMPORT_PATH: &str = "./tests/data/2024-08-24-2104.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/2024-08-24-2104.kml";
    let raw_vecotr_data1 = import_data::load_kml(IMPORT_PATH).unwrap();
    let vector1 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data1, false)
            .unwrap();

    export_data::journey_vector_to_kml_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();
    let raw_vecotr_data2 = import_data::load_kml(EXPORT_PATH).unwrap();
    let vector2 =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_vecotr_data2, false)
            .unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .collect_vec();

    assert_eq!(points1.len(), 10);
    assert_eq!(points1, points2);
    assert_f64_near!(points1[0].latitude, 36.6986802655);
    assert_f64_near!(points1[0].longitude, 117.1179554744);
}

#[test]
fn test_raw_data_csv_to_gpx_file() -> anyhow::Result<()> {
    const CSV_PATH: &str = "./tests/data/test_raw_data.csv";
    const GPX_EXPORT_PATH: &str = "./tests/for_inspection/test_raw_data_direct.gpx";

    let csv_file = File::open(CSV_PATH)?;
    let mut reader = csv::Reader::from_reader(BufReader::new(csv_file));

    let gpx_file = File::create(GPX_EXPORT_PATH)?;
    let mut writer = BufWriter::new(gpx_file);

    raw_data_csv_to_gpx_file(&mut reader, &mut writer)?;
    
    let gpx_file = File::open(GPX_EXPORT_PATH)?;
    let gpx = gpx::read(&mut BufReader::new(gpx_file))?;

    assert_eq!(gpx.tracks.len(), 1);

    let track_points: Vec<_> = gpx.tracks[0]
        .segments
        .iter()
        .flat_map(|seg| seg.points.iter())
        .collect_vec();

    assert_eq!(track_points.len(), 10);

    assert_f64_near!(track_points[0].point().x(), 36.6986802655);
    assert_f64_near!(track_points[0].point().y(), 117.1179554744);

    let metadata = gpx.metadata.expect("GPX metadata should exist");
    assert_eq!(metadata.name.as_deref(), Some("MemoLanes RawData"));
    Ok(())
}

