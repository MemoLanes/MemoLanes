use itertools::Itertools;
use memolanes_core::{export_data, import_data};
use std::fs::File;

#[test]
fn load_fow_sync_data() {
    let (bitmap_1, warnings_1) = import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let (bitmap_2, warnings_2) = import_data::load_fow_sync_data("./tests/data/fow_2.zip").unwrap();
    assert_eq!(bitmap_1, bitmap_2);
    assert_eq!(format!("{:?}", warnings_1), "None");
    assert_eq!(
        format!("{:?}", warnings_2),
        "Some(\"unexpected file: Sync/garbage_2\")"
    );
}

#[test]
pub fn gpx() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.gpx";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.gpx";
    let raw_vecotr_data1 = import_data::load_gpx(IMPORT_PATH).unwrap();
    let journey_info = import_data::journey_info_from_raw_vector_data(&raw_vecotr_data1);
    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();
    let vector1 = import_data::journey_vector_from_raw_data(raw_vecotr_data1, false).unwrap();
    export_data::journey_vector_to_gpx_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();

    let raw_vecotr_data2 = import_data::load_gpx(EXPORT_PATH).unwrap();
    let vector2 = import_data::journey_vector_from_raw_data(raw_vecotr_data2, false).unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter()
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter()
        .collect_vec();

    assert_eq!(points1.len(), 2945);
    assert_eq!(points1, points2);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
}

#[test]
pub fn kml() {
    const IMPORT_PATH: &str = "./tests/data/raw_gps_laojunshan.kml";
    const EXPORT_PATH: &str = "./tests/for_inspection/laojunshan.kml";
    let raw_vecotr_data1 = import_data::load_kml(IMPORT_PATH).unwrap();
    let journey_info = import_data::journey_info_from_raw_vector_data(&raw_vecotr_data1);
    let start_time = journey_info.start_time.unwrap().timestamp_millis();
    let end_time = journey_info.end_time.unwrap().timestamp_millis();
    let vector1 = import_data::journey_vector_from_raw_data(raw_vecotr_data1, false).unwrap();

    export_data::journey_vector_to_kml_file(&vector1, &mut File::create(EXPORT_PATH).unwrap())
        .unwrap();
    let raw_vecotr_data2 = import_data::load_kml(EXPORT_PATH).unwrap();
    let vector2 = import_data::journey_vector_from_raw_data(raw_vecotr_data2, false).unwrap();
    let tracks1 = vector1.track_segments;
    let tracks2 = vector2.track_segments;

    assert_eq!(tracks1.len(), 1);
    assert_eq!(tracks2.len(), tracks1.len());

    let points1 = tracks1
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter()
        .collect_vec();
    let points2 = tracks2
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter()
        .collect_vec();

    assert_eq!(points1.len(), 1651);
    assert_eq!(points1, points2);
    assert_eq!(start_time, 1696383677000);
    assert_eq!(end_time, 1696386835000);
}
