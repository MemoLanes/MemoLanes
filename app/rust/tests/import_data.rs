use memolanes_core::import_data;

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
pub fn import_gpx() {
    let vector = import_data::load_gpx("./tests/data/raw_gps_laojunshan.gpx", false).unwrap();
    let tracks = vector.track_segments;
    assert_eq!(tracks.len(), 1);
    let points = tracks
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter();
    assert_eq!(points.count(), 2945);
}

#[test]
pub fn import_kml() {
    let vector = import_data::load_kml("./tests/data/raw_gps_laojunshan.kml", false).unwrap();
    let tracks = vector.track_segments;
    assert_eq!(tracks.len(), 1);
    let points = tracks
        .into_iter()
        .flat_map(|t| t.track_points.into_iter())
        .into_iter();
    assert_eq!(points.count(), 1651);
}
