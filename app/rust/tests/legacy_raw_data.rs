#[macro_use]
extern crate assert_float_eq;

use std::{
    fs::{self, File},
    io::{BufReader, Cursor},
};

use itertools::Itertools;
use memolanes_core::legacy_raw_data::{
    delete_legacy_raw_data_file, export_legacy_raw_data_gpx_file, legacy_raw_data_csv_to_gpx_file,
    list_all_legacy_raw_data,
};
use tempdir::TempDir;

#[test]
fn lists_and_deletes_only_legacy_raw_data_csv_files() {
    let support_dir = TempDir::new("legacy-raw-data").unwrap();
    let raw_data_dir = support_dir.path().join("raw_data");
    fs::create_dir(&raw_data_dir).unwrap();
    fs::write(raw_data_dir.join("2024-02.csv"), b"csv").unwrap();
    fs::write(raw_data_dir.join("2024-01.csv"), b"csv").unwrap();
    fs::write(raw_data_dir.join("ignored.txt"), b"text").unwrap();

    let files = list_all_legacy_raw_data(support_dir.path().to_str().unwrap()).unwrap();
    assert_eq!(
        files.iter().map(|file| file.name.as_str()).collect_vec(),
        vec!["2024-02", "2024-01"]
    );

    delete_legacy_raw_data_file(support_dir.path().to_str().unwrap(), "2024-02").unwrap();
    let files = list_all_legacy_raw_data(support_dir.path().to_str().unwrap()).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "2024-01");
}

#[test]
fn legacy_raw_data_csv_to_gpx_preserves_optional_fields() {
    let csv = concat!(
        "timestamp_ms,received_timestamp_ms,latitude,longitude,accuracy,altitude,speed\n",
        ",1700000001234,31.230416,121.473701,3.5,42.5,\n",
        "0,1700000001234,31.230416,121.473701,,,\n",
        "-1,1700000001234,31.230416,121.473701,,,\n"
    );
    let mut reader = csv::Reader::from_reader(csv.as_bytes());
    let mut output = Cursor::new(Vec::new());

    legacy_raw_data_csv_to_gpx_file(&mut reader, &mut output).unwrap();
    output.set_position(0);
    let gpx = gpx::read(output).unwrap();
    let points = &gpx.tracks[0].segments[0].points;
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].elevation, Some(42.5));
    assert_eq!(points[0].hdop, Some(3.5));
    for point in &points[1..] {
        assert_eq!(
            (point.time, point.elevation, point.hdop),
            (None, None, None)
        );
    }
    let exported_time: time::OffsetDateTime = points[0].time.unwrap().into();

    assert_eq!(
        exported_time.unix_timestamp_nanos(),
        1_700_000_001_234_000_000
    );
}

#[test]
fn converts_legacy_raw_data_csv_to_gpx_file() {
    const CSV_PATH: &str = "./tests/data/raw_data.csv";
    const GPX_EXPORT_PATH: &str = "./tests/for_inspection/legacy_raw_data.gpx";

    let temp = TempDir::new("legacy-export-retry").unwrap();
    let csv_path = temp.path().join("retry.csv");
    let export = || {
        export_legacy_raw_data_gpx_file(csv_path.to_str().unwrap(), temp.path().to_str().unwrap())
    };
    let valid_csv = fs::read_to_string(CSV_PATH).unwrap();
    fs::write(&csv_path, format!("{valid_csv}\ninvalid,row\n")).unwrap();
    assert!(export().is_err());
    assert!(!temp.path().join("raw_data/retry.gpx").exists());
    fs::write(&csv_path, valid_csv).unwrap();
    let exported_path = export().unwrap();
    fs::copy(&exported_path, GPX_EXPORT_PATH).unwrap();

    let gpx_file = File::open(exported_path).unwrap();
    let gpx = gpx::read(&mut BufReader::new(gpx_file)).unwrap();
    assert_eq!(gpx.tracks.len(), 1);

    let track_points = gpx.tracks[0]
        .segments
        .iter()
        .flat_map(|segment| segment.points.iter())
        .collect_vec();
    assert_eq!(track_points.len(), 929);
    assert_f64_near!(track_points[0].point().x(), -0.104277);
    assert_f64_near!(track_points[0].point().y(), 51.520302);
    assert_eq!(
        gpx.metadata.unwrap().name.as_deref(),
        Some("MemoLanes Raw Data")
    );
}
