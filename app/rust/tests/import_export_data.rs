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
use memolanes_core::journey_header::{JourneyInfo, JourneyKind};
use memolanes_core::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use memolanes_core::main_db::{MainDb, NewJourney};
use memolanes_core::raw_data::{JourneyRawData, SerializedJourneyRawData};
use memolanes_core::{export_data, import_data};
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
fn gpx_accepts_positive_dateline_from_any_source() {
    let temp = tempdir::TempDir::new("gpx-dateline").unwrap();
    let path = temp.path().join("dateline.gpx");
    // Cover each source and equivalent longitude spelling without a full
    // Cartesian product: coordinate normalization is independent of source.
    for (creator, longitude) in [
        ("MemoLanes", "180"),
        ("OtherApp", "180.0"),
        ("StepOfMyWorld", "+180"),
        ("yourapp", "1.8e2"),
        ("OtherApp", "&#49;80"),
    ] {
        let xml = format!(
            r#"<gpx version="1.1" creator="{creator}" xmlns="http://www.topografix.com/GPX/1/1"
                  xmlns:ml="https://app.memolanes.com/ns/journey/1">
                  <wpt lat="3" lon="{longitude}"><name>East &amp; West</name></wpt>
                  <rte><rtept lat="2" lon="{longitude}"/></rte>
                  <trk><extensions><ml:journey date="2000-01-01"/></extensions>
                    <trkseg><trkpt lat="1" lon="179.9"/>
                      <trkpt lat="1" lon="{longitude}"><ele>12</ele></trkpt></trkseg>
                    <trkseg><trkpt lat="1" lon="-180"/><trkpt lat="1" lon="-179.9"/></trkseg>
                  </trk>
                </gpx>"#
        );
        fs::write(&path, &xml).unwrap();
        let (parsed, preprocessor) = load_vector_fixture(&path);
        // Keep the existing automatic processor selection unchanged.
        assert!(match creator {
            "StepOfMyWorld" | "yourapp" => matches!(preprocessor, ImportPreprocessor::Spare),
            _ => matches!(preprocessor, ImportPreprocessor::Generic),
        });
        let segments = parsed.flatten();
        assert_eq!(segments.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 1]);
        assert_eq!(segments[0][0].point.longitude, 179.9);
        assert_eq!(segments[1][0].point.longitude, -180.0);
        assert_eq!(segments[1][1].point.longitude, -179.9);
        assert_eq!(segments[0][1].altitude, Some(12.0));
        for point in [&segments[0][1], &segments[2][0]] {
            assert!(point.point.longitude < 180.0);
            assert!((point.point.longitude - 180.0).abs() < 1e-12);
        }
        let vector = import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(
            &segments, None,
        )
        .unwrap();
        assert_eq!(vector.track_segments.len(), 3);

        // Waypoints share the same preparation even though the trajectory
        // importer only consumes tracks and routes.
        let (prepared, _) = memolanes_core::gpx_file_utils::analyze_and_prepare_gpx(&xml).unwrap();
        let gpx = gpx::read(prepared.as_bytes()).unwrap();
        assert_eq!(gpx.waypoints.len(), 1);
        assert_eq!(gpx.waypoints[0].name.as_deref(), Some("East & West"));
        assert!(gpx.waypoints[0].point().x() < 180.0);
        assert!((gpx.waypoints[0].point().x() - 180.0).abs() < 1e-12);
    }
}

#[test]
fn gpx_still_rejects_out_of_range_longitudes() {
    let temp = tempdir::TempDir::new("gpx-invalid-longitude").unwrap();
    let path = temp.path().join("invalid.gpx");
    for longitude in ["180.1", "-180.1", "NaN", "inf"] {
        fs::write(
            &path,
            format!(
                r#"<gpx version="1.1" creator="OtherApp" xmlns="http://www.topografix.com/GPX/1/1">
              <trk><trkseg><trkpt lat="1" lon="{longitude}"/></trkseg></trk>
            </gpx>"#
            ),
        )
        .unwrap();
        assert!(
            import_data::gpx::load_gpx(path.to_str().unwrap()).is_err(),
            "{longitude}"
        );
    }
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
    for (path, processed_journey) in [
        (path.as_path(), true),
        (
            Path::new("./tests/data/memolanes_foreign_extensions.gpx"),
            false,
        ),
    ] {
        let label = path.display();
        let (parsed, preprocessor) = load_vector_fixture(path);
        assert_eq!(
            matches!(preprocessor, ImportPreprocessor::None),
            processed_journey,
            "{label}"
        );
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
fn kml_prefers_tracks_across_placemarks_and_falls_back_without_tracks() {
    let temp = tempdir::TempDir::new("kml-track-priority").unwrap();
    let path = temp.path().join("other-app.kml");
    let track = r#"<Placemark><gx:Track>
        <when>2026-09-11T01:00:00Z</when><when>2026-09-11T01:00:01Z</when>
        <gx:coord>120 30 10</gx:coord><gx:coord>120.001 30.001 11</gx:coord>
      </gx:Track>
      <LineString><coordinates>120,30 120.001,30.001</coordinates></LineString>
    </Placemark>"#;
    // Identical Tracks remain separate, including across Folder boundaries.
    let tracks = format!("{track}<Folder>{track}</Folder>");
    let legacy_metadata = r#"<ExtendedData><Data name="memolanes:date"><value>2000-01-01</value></Data></ExtendedData>"#;
    for (label, tracks, metadata, has_tracks) in [
        ("no Track", "", "", false),
        (
            "empty Track",
            "<Placemark><gx:Track/></Placemark>",
            "",
            false,
        ),
        ("ordinary KML", tracks.as_str(), "", true),
        ("legacy ML metadata", tracks.as_str(), legacy_metadata, true),
    ] {
        fs::write(
            &path,
            format!(
                r#"<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
                  <Document>
                    <Placemark><LineString><coordinates>120,30 120.001,30.001</coordinates></LineString></Placemark>
                    {tracks}
                    <Folder>{metadata}<Placemark><LineString><coordinates>121,31 121.001,31.001</coordinates></LineString></Placemark></Folder>
                    <Placemark><Point><coordinates>122,32</coordinates></Point></Placemark>
                  </Document>
                </kml>"#
            ),
        )
        .unwrap();
        let (parsed, preprocessor) = load_vector_fixture(&path);
        assert!(matches!(preprocessor, ImportPreprocessor::Generic));
        let segments = parsed.flatten();
        let coordinates = segments
            .iter()
            .map(|segment| {
                segment
                    .iter()
                    .map(|point| (point.point.longitude, point.point.latitude))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let first_line = vec![(120.0, 30.0), (120.001, 30.001)];
        if has_tracks {
            assert_eq!(coordinates, [first_line.clone(), first_line], "{label}");
            assert_eq!(segments[0], segments[1]);
            assert_eq!(segments[0][0].altitude, Some(10.0));
            assert_eq!(
                segments[0][0].timestamp_ms,
                Some(
                    DateTime::parse_from_rfc3339("2026-09-11T01:00:00Z")
                        .unwrap()
                        .timestamp_millis()
                )
            );
        } else {
            assert_eq!(
                coordinates,
                [
                    first_line,
                    vec![(121.0, 31.0), (121.001, 31.001)],
                    vec![(122.0, 32.0)]
                ],
                "{label}"
            );
            assert!(segments
                .iter()
                .flatten()
                .all(|point| point.timestamp_ms.is_none()));
        }
    }
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
fn bulk_export_preserves_geometry_and_reimports_as_ordinary_data() {
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
    // Both formats retain coordinates and segment boundaries, without a custom
    // reimport protocol. Untimed tracks follow the ordinary single-import flow.
    for (format, path, result) in [
        (ExportType::GPX, &gpx_path, export_gpx()),
        (ExportType::KML, &kml_path, export_kml()),
    ] {
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        let xml = fs::read_to_string(path).unwrap();
        assert!(!xml.contains("memolanes:"));
        for id in &ids {
            assert!(!xml.contains(id));
        }
        let (parsed, preprocessor) = load_vector_fixture(path);
        assert!(matches!(preprocessor, ImportPreprocessor::None));
        let segments = parsed.flatten();
        assert_eq!(
            segments.iter().map(Vec::len).collect::<Vec<_>>(),
            [2, 1, 2, 1, 2, 1]
        );
        let mut offsets = Vec::new();
        for pair in segments.chunks_exact(2) {
            let offset = pair[0][0].point.latitude;
            offsets.push(offset as i32);
            assert_eq!(
                pair.iter()
                    .map(|segment| segment
                        .iter()
                        .map(|point| (
                            point.point.latitude,
                            point.point.longitude,
                            point.timestamp_ms,
                        ))
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                [
                    vec![(offset, 1.0, None), (offset + 1.0, 2.0, None)],
                    vec![(offset + 2.0, 3.0, None)]
                ]
            );
        }
        offsets.sort_unstable();
        assert_eq!(offsets, [1, 10, 20]);

        let extension = path.extension().unwrap().to_str().unwrap();
        let single = temp.path().join(format!("single.{extension}"));
        assert!(matches!(
            export_journey(
                single.to_string_lossy().into_owned(),
                ids[0].clone(),
                format,
                false,
            )
            .unwrap(),
            ExportResult::Succeed
        ));
        let single_xml = fs::read_to_string(&single).unwrap();
        assert!(!single_xml.contains("memolanes:"));
        let (single_parsed, preprocessor) = load_vector_fixture(&single);
        assert!(matches!(preprocessor, ImportPreprocessor::None));
        let single_segments = single_parsed.flatten();
        assert_eq!(
            single_segments.iter().map(Vec::len).collect::<Vec<_>>(),
            [2, 1]
        );
        assert!(segments.windows(2).any(|pair| pair == single_segments));

        // Old MemoLanes metadata is ignored just like any third-party extension.
        let legacy_xml = match format {
            ExportType::GPX => xml.replace("<gpx ", "<gpx xmlns:memolanes=\"https://app.memolanes.com/ns/journey/1\" ")
                .replace("</type>", "</type><extensions><memolanes:journey date=\"2000-01-01\" start=\"invalid\" sourceJourneyId=\"old-id\" sourceRevision=\"old-revision\"/></extensions>"),
            ExportType::KML => xml.replace("<Folder>", "<Folder><ExtendedData><Data name=\"memolanes:date\"><value>2000-01-01</value></Data><Data name=\"memolanes:sourceJourneyId\"><value>old-id</value></Data></ExtendedData>"),
            _ => unreachable!(),
        };
        let legacy_path = temp.path().join(format!("legacy.{extension}"));
        fs::write(&legacy_path, legacy_xml).unwrap();
        assert_eq!(parse_vector(&legacy_path), parsed);
        for input in [path, &legacy_path] {
            let original_headers = db
                .with_txn(|txn| txn.query_journeys(None, None, None))
                .unwrap();
            for requested in [ImportPreprocessor::None, ImportPreprocessor::Generic] {
                let (mut info, raw, preprocessor) =
                    import_api::load_vector_data(input.to_string_lossy().into_owned()).unwrap();
                assert!(matches!(preprocessor, ImportPreprocessor::None));
                assert!(import_api::analyze_vector_data_parts(&raw).is_empty());
                assert_eq!(info.start_time, None);
                assert_eq!(info.end_time, None);
                info.note = Some("ordinary reimport".into());
                // Detection only supplies a default; callers can override it.
                let data = import_api::process_vector_data(&raw, requested).unwrap();
                import_api::import_journey_data(info, data).unwrap();
            }
            db.with_txn(|txn| {
                let headers = txn.query_journeys(None, None, None)?;
                assert_eq!(headers.len(), original_headers.len() + 2);
                for header in &original_headers {
                    assert_eq!(txn.get_journey_header(&header.id)?, Some(header.clone()));
                }
                for header in headers
                    .iter()
                    .filter(|header| !original_headers.iter().any(|old| old.id == header.id))
                {
                    let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
                        panic!("ordinary import is not vector data");
                    };
                    assert_eq!(vector.track_segments.len(), segments.len());
                    for (actual, expected) in vector.track_segments.iter().zip(&segments) {
                        assert_eq!(
                            actual.track_points,
                            expected
                                .iter()
                                .map(|point| TrackPoint {
                                    latitude: point.point.latitude,
                                    longitude: point.point.longitude,
                                })
                                .collect::<Vec<_>>()
                        );
                    }
                    txn.delete_journey(&header.id)?;
                }
                Ok(())
            })
            .unwrap();
        }
    }

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
            assert!(parts.iter().all(|part| part.is_explicit_journey));
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
            // Timed GPX/KML use the same date-based multi-import path even if
            // they carry legacy MemoLanes metadata. Reimports always add copies.
            let legacy_xml = match format {
                RawDataExportType::GPX => text.replace("<gpx ", "<gpx xmlns:memolanes=\"https://app.memolanes.com/ns/journey/1\" ")
                    .replace("</type>", "</type><extensions><memolanes:journey date=\"2000-01-01\"/></extensions>"),
                RawDataExportType::KML => text.replace("<Folder>", "<Folder><ExtendedData><Data name=\"memolanes:date\"><value>2000-01-01</value></Data></ExtendedData>"),
                _ => unreachable!(),
            };
            let legacy_path = temp.path().join(format!("legacy-raw.{extension}"));
            fs::write(&legacy_path, legacy_xml).unwrap();
            assert_eq!(parse_vector(&legacy_path), parsed);
            let (plain_info, plain_raw, _) =
                import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
            let plain_parts = import_api::analyze_vector_data_parts(&plain_raw);
            let (info, raw, preprocessor) =
                import_api::load_vector_data(legacy_path.to_string_lossy().into_owned()).unwrap();
            assert!(matches!(preprocessor, ImportPreprocessor::Generic));
            assert_eq!(info.journey_date, plain_info.journey_date);
            assert_eq!(info.start_time, plain_info.start_time);
            assert_eq!(info.end_time, plain_info.end_time);
            let parts = import_api::analyze_vector_data_parts(&raw);
            assert!(!parts.is_empty());
            assert_eq!(parts.len(), plain_parts.len());
            for (part, plain) in parts.iter().zip(&plain_parts) {
                assert!(!part.is_explicit_journey);
                assert_eq!(part.part_key, plain.part_key);
                assert_eq!(part.journey_date, plain.journey_date);
                assert_eq!(part.start_time, plain.start_time);
                assert_eq!(part.end_time, plain.end_time);
                assert_eq!(part.point_count, plain.point_count);
            }
            let initial_count = db
                .with_txn(|txn| Ok(txn.query_journeys(None, None, None)?.len()))
                .unwrap();
            for _ in 0..2 {
                assert_eq!(
                    import_api::import_vector_data_by_parts(
                        &raw,
                        parts.iter().map(|part| part.part_key.clone()).collect(),
                        preprocessor,
                        JourneyKind::DefaultKind,
                        None,
                    )
                    .unwrap(),
                    parts.len() as u64
                );
            }
            db.with_txn(|txn| {
                assert_eq!(
                    txn.query_journeys(None, None, None)?.len(),
                    initial_count + 2 * parts.len()
                );
                Ok(())
            })
            .unwrap();
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
