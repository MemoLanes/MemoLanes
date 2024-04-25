use std::fs::File;

use itertools::Itertools;
use memolanes_core::{export_data, import_data};

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
    let vector1 = import_data::load_gpx("./tests/data/raw_gps_laojunshan.gpx", false).unwrap();
    export_data::journey_vector_to_gpx_file(
        &vector1,
        &mut File::create("./tests/for_inspection/laojunshan.gpx").unwrap(),
    )
    .unwrap();
    let vector2 = import_data::load_gpx("./tests/for_inspection/laojunshan.gpx", false).unwrap();
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
}

#[test]
pub fn kml() {
    let vector1 = import_data::load_kml("./tests/data/raw_gps_laojunshan.kml", false).unwrap();

    export_data::journey_vector_to_kml_file(
        &vector1,
        &mut File::create("./tests/for_inspection/laojunshan.kml").unwrap(),
    )
    .unwrap();
    let vector2 = import_data::load_kml("./tests/for_inspection/laojunshan.kml", false).unwrap();
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
}
