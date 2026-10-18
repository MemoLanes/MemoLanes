#[macro_use]
extern crate assert_float_eq;

pub mod test_utils;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use memolanes_core::api::api::{
    delete_journey_raw_data, export_all_journeys_as_fwss, export_all_journeys_as_gpx,
    export_all_journeys_as_kml, export_all_journeys_raw_data, export_journey,
    export_journey_raw_data, generate_full_archive, get_journey_detail, has_journeys_raw_data,
    init, rebuild_cache, ExportResult, ExportType, MapRendererProxy,
};
use memolanes_core::api::import as import_api;
use memolanes_core::archive::MldxReader;
use memolanes_core::export_data::RawDataExportType;
use memolanes_core::gps::{ExtendedRawGPSPoint, Point, RawGPSPoint};
use memolanes_core::gpx_file_utils::{normalize_generic_time, normalize_step_of_my_world_time};
use memolanes_core::import_data::ImportPreprocessor;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::JourneyInfo;
use memolanes_core::journey_header::JourneyKind;
use memolanes_core::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use memolanes_core::main_db::{MainDb, NewJourney};
use memolanes_core::raw_data::{JourneyRawData, SerializedJourneyRawData};
use memolanes_core::{export_data, import_data};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::Path;

fn load_vector_fixture(path: &Path) -> (Vec<Vec<RawGPSPoint>>, ImportPreprocessor) {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("gpx") => import_data::gpx::load_gpx(path.to_str().unwrap()),
        Some("kml") => import_data::kml::load_kml(path.to_str().unwrap()),
        Some("csv") => import_data::csv::load_csv(path.to_str().unwrap()),
        _ => panic!("Unsupported vector fixture: {}", path.display()),
    }
    .unwrap()
}

fn run_vector_integrity_check(
    import_path: &str,
    export_path: &str,
) -> (Vec<TrackPoint>, JourneyInfo, ImportPreprocessor) {
    let (raw_data1, preprocessor) = load_vector_fixture(Path::new(import_path));
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

    let (raw_data2, _) = load_vector_fixture(Path::new(export_path));
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
fn vector_fixtures_roundtrip() {
    use ImportPreprocessor::{Generic, Spare};
    let temp = tempdir::TempDir::new("vector-roundtrip").unwrap();
    for (file, count, times, first_point, expected_processor) in [
        (
            "raw_gps_laojunshan.gpx",
            2945,
            Some((1696383677000, 1696386835000)),
            None,
            Generic,
        ),
        (
            "StepOfMyWorld.gpx",
            313,
            Some((1766592001000, 1766678400000)),
            None,
            Spare,
        ),
        (
            "yourapp.gpx",
            400,
            Some((1766624275000, 1766665298000)),
            None,
            Spare,
        ),
        ("2bulu.gpx", 53, None, None, Generic),
        (
            "2bulu.kml",
            53,
            None,
            Some((36.4375453, 116.9442243)),
            Generic,
        ),
        (
            "raw_gps_laojunshan.kml",
            1651,
            Some((1696383677000, 1696386835000)),
            None,
            Generic,
        ),
        (
            "2024-08-24-2104.kml",
            10,
            None,
            Some((36.6986802655, 117.1179554744)),
            Generic,
        ),
    ] {
        eprintln!("vector roundtrip: {file}");
        let (points, info, processor) = run_vector_integrity_check(
            &format!("./tests/data/{file}"),
            temp.path().join(file).to_str().unwrap(),
        );
        assert_eq!(points.len(), count, "{file}");
        assert_eq!(
            std::mem::discriminant(&processor),
            std::mem::discriminant(&expected_processor),
            "{file}"
        );
        if let Some((start, end)) = times {
            assert_eq!(info.start_time.unwrap().timestamp_millis(), start, "{file}");
            assert_eq!(info.end_time.unwrap().timestamp_millis(), end, "{file}");
        }
        if let Some((latitude, longitude)) = first_point {
            assert_f64_near!(points[0].latitude, latitude);
            assert_f64_near!(points[0].longitude, longitude);
        }
    }
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
        let segments = parsed;
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
        let segments = parsed;
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
        let segments = parsed;
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

#[test]
fn preprocessor_selection_by_format_source_and_label() {
    use ImportPreprocessor::{Generic, None as NoPreprocessor, Spare};
    let temp = tempdir::TempDir::new("preprocessor").unwrap();
    for (source, label, gpx_processor, kml_processor) in [
        (
            "MemoLanes",
            "MemoLanes Journey 2026-10-14",
            NoPreprocessor,
            NoPreprocessor,
        ),
        ("MemoLanes", "", Generic, Generic),
        ("Other App", "", Generic, Generic),
        ("MemoLanes", "MemoLanes Raw Data", Generic, Generic),
        (
            "MemoLanes",
            "MemoLanes Journey; MemoLanes Raw Data",
            Generic,
            Generic,
        ),
        ("StepOfMyWorld", "", Spare, Generic),
        ("YourApp", "", Spare, Generic),
        ("StepOfMyWorld", "MemoLanes Journey", Spare, NoPreprocessor),
        ("YourApp", "MemoLanes Journey", Spare, NoPreprocessor),
    ] {
        for (extension, expected) in [("gpx", gpx_processor), ("kml", kml_processor)] {
            eprintln!("preprocessor: {extension}, {source}, {label}");
            let path = temp.path().join(format!("track.{extension}"));
            let xml = match extension {
                "gpx" => format!(
                    r#"<gpx version="1.1" creator="{source}" xmlns="http://www.topografix.com/GPX/1/1"><trk><name>{label}</name><trkseg><trkpt lat="31" lon="121"/></trkseg></trk></gpx>"#
                ),
                "kml" => format!(
                    r#"<kml xmlns="http://www.opengis.net/kml/2.2"><Document><name>{source}</name><Placemark><name>{label}</name><Point><coordinates>121,31</coordinates></Point></Placemark></Document></kml>"#
                ),
                _ => unreachable!(),
            };
            fs::write(&path, xml).unwrap();
            let (_, _, actual) =
                import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
            assert_eq!(
                std::mem::discriminant(&actual),
                std::mem::discriminant(&expected),
                "{extension}: {source}, {label}"
            );
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

fn export_test_raw_data(latitude: f64, start: DateTime<Utc>) -> JourneyRawData {
    let points = (0..3)
        .map(|index| ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                point: Point {
                    latitude: latitude + index as f64 * 0.0001,
                    longitude: 121.473701,
                },
                timestamp_ms: Some(start.timestamp_millis() + index * 10_000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            received_timestamp_ms: start.timestamp_millis() + index * 10_000 + 1234,
        })
        .collect();
    JourneyRawData::new(points, start.timestamp_millis())
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
    for extension in ["mldx", "fwss"] {
        let path = temp.path().join(format!("empty.{extension}"));
        let filepath = path.to_string_lossy().into_owned();
        let result = if extension == "mldx" {
            generate_full_archive(filepath, true)
        } else {
            export_all_journeys_as_fwss(filepath)
        };
        assert!(matches!(result.unwrap(), ExportResult::DataIsEmpty));
        assert!(!path.exists());
    }
    // The bridge owns a process-wide OnceLock, so initialize it once and
    // exercise the vector and raw-data scenarios against the same database.
    check_vector_exports(temp.path(), &mut db);
    check_raw_exports(temp.path(), &support, &mut db);
}

fn check_journey_detail_and_mldx_analysis(path: &Path, db: &mut MainDb) {
    use import_api::MldxJourneyAnalysis;
    use import_api::MldxJourneyImportAnalyzeResult::{Conflict, New, Unchanged};

    // The single-journey archive was exported before any local metadata edits.
    let reader = import_api::OpaqueMldxReader::open(path.to_string_lossy().into_owned()).unwrap();
    let initial = reader.analyze().unwrap();
    assert_eq!(initial.len(), 1);
    let archived_header = initial[0].imported_header.clone();
    assert_eq!(
        initial,
        [MldxJourneyAnalysis {
            imported_header: archived_header.clone(),
            status: Unchanged,
            local_header: None,
        }]
    );

    let local_header = db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &archived_header.id,
                archived_header.journey_date,
                archived_header.start,
                archived_header.end,
                Some("Local metadata changed".to_owned()),
                archived_header.journey_kind,
            )?;
            Ok(txn.get_journey_header(&archived_header.id)?.unwrap())
        })
        .unwrap();
    assert_eq!(
        reader.analyze().unwrap(),
        [MldxJourneyAnalysis {
            imported_header: archived_header.clone(),
            status: Conflict,
            local_header: Some(local_header.clone()),
        }]
    );
    let (header, renderer, bounds) = get_journey_detail(&archived_header.id).unwrap().unwrap();
    assert_eq!(header, local_header);
    assert!(bounds.is_some());
    let MapRendererProxy::DynamicRenderer(renderer) = renderer else {
        panic!("Expected a detail renderer");
    };
    let mut expected_bitmap = JourneyBitmap::new();
    export_test_vector(1.0).merge_into(&mut expected_bitmap);
    assert_eq!(
        renderer.lock().unwrap().peek_latest_bitmap(),
        &expected_bitmap
    );

    db.with_txn(|txn| txn.delete_journey(&archived_header.id))
        .unwrap();
    assert!(get_journey_detail(&archived_header.id).unwrap().is_none());
    assert_eq!(
        reader.analyze().unwrap(),
        [MldxJourneyAnalysis {
            imported_header: archived_header.clone(),
            status: New,
            local_header: None,
        }]
    );
    db.with_txn(|txn| txn.insert_journey(archived_header, export_test_vector(1.0)))
        .unwrap();
}

fn check_vector_exports(temp: &Path, db: &mut MainDb) {
    let gpx_path = temp.join("all.gpx");
    let kml_path = temp.join("all.kml");
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
                        start: Some(journey_date.and_time(start.time()).and_utc()),
                        end: Some(journey_date.and_time(end.time()).and_utc()),
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
    // Fixtures use a separate connection, so refresh the cache before the API export.
    rebuild_cache().unwrap();
    let fwss_path = temp.join("journeys.fwss");
    assert!(matches!(
        export_all_journeys_as_fwss(fwss_path.to_string_lossy().into_owned()).unwrap(),
        ExportResult::Succeed
    ));
    let (bitmap, warnings) =
        import_data::fow::load_fow_snapshot_data(fwss_path.to_str().unwrap()).unwrap();
    let mut expected_bitmap = JourneyBitmap::new();
    for offset in [1.0, 10.0, 20.0] {
        export_test_vector(offset).merge_into(&mut expected_bitmap);
    }
    assert_eq!(bitmap, expected_bitmap);
    assert!(warnings.is_none());
    // Read back real MLDX exports to cover buffered writes and seeking.
    let mldx_path = temp.join("journeys.mldx");
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
    check_journey_detail_and_mldx_analysis(&mldx_path, db);
    // Both formats retain coordinates and segment boundaries, without a custom
    // reimport protocol. Untimed tracks follow the ordinary single-import flow.
    for (format, path, result) in [
        (ExportType::GPX, &gpx_path, export_gpx()),
        (ExportType::KML, &kml_path, export_kml()),
    ] {
        eprintln!("vector export and reimport: {format:?}");
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        let xml = fs::read_to_string(path).unwrap();
        assert!(!xml.contains("memolanes:"));
        for id in &ids {
            assert!(!xml.contains(id));
        }
        let (segments, preprocessor) = load_vector_fixture(path);
        assert!(matches!(preprocessor, ImportPreprocessor::None));
        assert_eq!(
            segments.iter().map(Vec::len).collect::<Vec<_>>(),
            [2, 1, 2, 1, 2, 1]
        );
        let mut offsets = Vec::new();
        for pair in segments.as_chunks::<2>().0 {
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
        let same_day_offsets = if ids[0] < ids[1] { [1, 10] } else { [10, 1] };
        assert_eq!(offsets, [same_day_offsets[0], same_day_offsets[1], 20]);

        let extension = path.extension().unwrap().to_str().unwrap();
        let single = temp.join(format!("single.{extension}"));
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
        let single_segments = single_parsed;
        assert_eq!(
            single_segments.iter().map(Vec::len).collect::<Vec<_>>(),
            [2, 1]
        );
        assert!(segments.windows(2).any(|pair| pair == single_segments));

        let original_headers = db
            .with_txn(|txn| txn.query_journeys(None, None, None))
            .unwrap();
        for requested in [ImportPreprocessor::None, ImportPreprocessor::Generic] {
            let (mut info, raw, preprocessor) =
                import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
            assert!(matches!(preprocessor, ImportPreprocessor::None));
            assert!(import_api::analyze_vector_data_by_date(&raw).is_empty());
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

fn check_raw_reimport(path: &Path, db: &mut MainDb, expected: &BTreeMap<NaiveDate, JourneyVector>) {
    let (_, raw, processor) =
        import_api::load_vector_data(path.to_string_lossy().into_owned()).unwrap();
    assert!(matches!(processor, ImportPreprocessor::Generic));
    let parts = import_api::analyze_vector_data_by_date(&raw);
    assert_eq!(
        parts
            .iter()
            .map(|part| (part.journey_date.clone(), part.point_count))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(date, vector)| (
                date.to_string(),
                vector
                    .track_segments
                    .iter()
                    .flat_map(|segment| &segment.track_points)
                    .count() as u64,
            ))
            .collect::<Vec<_>>()
    );
    let original_headers = db
        .with_txn(|txn| txn.query_journeys(None, None, None))
        .unwrap();
    // Repeated imports add copies; raw formats do not carry identity for deduplication.
    for _ in 0..2 {
        assert_eq!(
            import_api::import_vector_data_by_date(
                &raw,
                parts.iter().map(|part| part.journey_date.clone()).collect(),
                processor,
                JourneyKind::DefaultKind,
                None,
            )
            .unwrap(),
            expected.len() as u64
        );
    }
    db.with_txn(|txn| {
        let headers = txn.query_journeys(None, None, None)?;
        assert_eq!(headers.len(), original_headers.len() + 2 * expected.len());
        for header in &original_headers {
            assert_eq!(txn.get_journey_header(&header.id)?, Some(header.clone()));
        }
        for header in headers
            .iter()
            .filter(|header| !original_headers.iter().any(|old| old.id == header.id))
        {
            assert!(!header.has_raw_data);
            let JourneyData::Vector(vector) = txn.get_journey_data(&header.id)? else {
                panic!("{} import is not vector data", path.display());
            };
            assert_eq!(&vector, &expected[&header.journey_date]);
            txn.delete_journey(&header.id)?;
        }
        Ok(())
    })
    .unwrap();
}

fn check_raw_exports(temp: &Path, support: &str, db: &mut MainDb) {
    let first_date = NaiveDate::from_ymd_opt(2026, 9, 11).unwrap();
    let second_date = first_date.succ_opt().unwrap();
    // Raw exports include only non-empty v2 attachments, even without vector data.
    let raw_formats = [
        (RawDataExportType::CSV, "csv"),
        (RawDataExportType::GPX, "gpx"),
        (RawDataExportType::KML, "kml"),
    ];
    let export_raw = |format, extension: &str| {
        let path = temp.join(format!("raw.{extension}"));
        (
            export_all_journeys_raw_data(path.to_string_lossy().into_owned(), format),
            path,
        )
    };
    let assert_empty_raw = |has_raw_data| {
        assert_eq!(has_journeys_raw_data().unwrap(), has_raw_data);
        for (format, extension) in raw_formats {
            let (result, path) = export_raw(format, extension);
            assert!(matches!(result.unwrap(), ExportResult::DataIsEmpty));
            assert!(!path.exists());
        }
    };
    assert_empty_raw(false);
    fs::create_dir(format!("{support}/raw_data")).unwrap();
    fs::copy(
        "./tests/data/raw_data.csv",
        format!("{support}/raw_data/2024-01.csv"),
    )
    .unwrap();
    assert!(!has_journeys_raw_data().unwrap());
    let insert_raw = |db: &mut MainDb, date, start, raw_data: SerializedJourneyRawData| {
        db.with_txn(|txn| {
            txn.create_and_insert_journey(NewJourney {
                journey_date: date,
                start,
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
    let empty_raw_data = JourneyRawData::new(Vec::new(), 1234).serialize().unwrap();
    let empty_raw_id = insert_raw(db, first_date, None, empty_raw_data.clone());
    assert_empty_raw(true);

    // MLDX preserves header-only attachments, even when raw format exports are empty.
    let expected_header = db
        .with_txn(|txn| Ok(txn.get_journey_header(&empty_raw_id)?.unwrap()))
        .unwrap();
    let mldx_path = temp.join("empty-raw.mldx");
    assert!(matches!(
        generate_full_archive(mldx_path.to_string_lossy().into_owned(), true).unwrap(),
        ExportResult::Succeed
    ));
    let mut archive = MldxReader::open(File::open(mldx_path).unwrap()).unwrap();
    let (header, _, raw_data) = archive
        .load_single_journey_with_raw_data(&empty_raw_id)
        .unwrap()
        .unwrap();
    assert_eq!(header, expected_header);
    assert_eq!(raw_data, Some(empty_raw_data));

    // Cover same-day and cross-day ordering, including a manually edited display date.
    let recordings = [
        (second_date, first_date, 9, 31.230416),
        (first_date, first_date, 11, 32.230416),
        (second_date, second_date, 9, 33.230416),
    ]
    .map(|(display_date, date, hour, latitude)| {
        let start = Local
            .from_local_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
            .single()
            .unwrap()
            .with_timezone(&Utc);
        (display_date, start, export_test_raw_data(latitude, start))
    });
    let mut ids = Vec::new();
    // Expected geometry comes directly from the input recordings, without
    // running the production preprocessor to generate the test's oracle.
    let mut expected = BTreeMap::<NaiveDate, JourneyVector>::new();
    for (date, start, data) in &recordings {
        ids.push(insert_raw(
            db,
            *date,
            Some(*start),
            data.serialize().unwrap(),
        ));
        expected
            .entry(start.with_timezone(&Local).date_naive())
            .or_insert_with(|| JourneyVector {
                track_segments: Vec::new(),
            })
            .track_segments
            .push(TrackSegment {
                track_points: data
                    .points
                    .iter()
                    .map(|point| TrackPoint {
                        latitude: point.raw_gps_point.point.latitude,
                        longitude: point.raw_gps_point.point.longitude,
                    })
                    .collect(),
            });
    }
    let listed_ids = db
        .with_txn(|txn| {
            Ok(txn
                .query_journeys(None, None, None)?
                .into_iter()
                .filter(|header| ids.contains(&header.id))
                .map(|header| header.id)
                .collect::<Vec<_>>())
        })
        .unwrap();
    assert_eq!(listed_ids, [ids[2].clone(), ids[0].clone(), ids[1].clone()]);
    let expected_points = recordings
        .iter()
        .flat_map(|(_, _, data)| &data.points)
        .map(|point| {
            let raw = &point.raw_gps_point;
            (raw.timestamp_ms, raw.point.latitude, raw.point.longitude)
        })
        .collect::<Vec<_>>();
    assert!(has_journeys_raw_data().unwrap());
    for (format, extension) in raw_formats {
        let (result, path) = export_raw(format, extension);
        assert!(matches!(result.unwrap(), ExportResult::Succeed));
        eprintln!("raw export and reimport: {extension}");
        let is_csv = format == RawDataExportType::CSV;
        let text = fs::read_to_string(&path).unwrap();
        assert!(ids.iter().all(|id| !text.contains(id)));
        let segments = load_vector_fixture(&path).0;
        if is_csv {
            let mut reader = csv::Reader::from_path(&path).unwrap();
            let headers = reader.headers().unwrap();
            assert_eq!(
                headers.iter().collect::<Vec<_>>(),
                [
                    "timestamp_ms",
                    "received_timestamp_ms",
                    "latitude",
                    "longitude",
                    "accuracy",
                    "altitude",
                    "speed",
                ]
            );
            let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
            assert_eq!(rows.len(), 9);
        } else {
            for (index, (date, _, _)) in recordings.iter().enumerate() {
                assert!(text.contains(&format!("MemoLanes Raw Data {date} #{}", index + 1)));
            }
        }
        assert_eq!(
            segments.iter().map(Vec::len).collect::<Vec<_>>(),
            if is_csv { vec![9] } else { vec![3, 3, 3] }
        );
        assert_eq!(
            segments
                .iter()
                .flatten()
                .map(|point| (
                    point.timestamp_ms,
                    point.point.latitude,
                    point.point.longitude,
                ))
                .collect::<Vec<_>>(),
            expected_points
        );
        check_raw_reimport(&path, db, &expected);
    }

    // Fail after writing valid journeys, remove partial files, then refresh availability.
    let corrupt_id = insert_raw(
        db,
        second_date.succ_opt().unwrap(),
        None,
        SerializedJourneyRawData::from_bytes(b"invalid".to_vec()),
    );
    for (format, extension) in raw_formats {
        let (result, path) = export_raw(format, extension);
        assert!(result.is_err());
        assert!(!path.exists());
    }
    // Single exports must also remove files when a writer rejects a GPS timestamp.
    let mut invalid_raw = export_test_raw_data(31.0, recordings[0].1);
    invalid_raw.points[1].raw_gps_point.timestamp_ms = Some(i64::MAX);
    let invalid_id = insert_raw(db, first_date, None, invalid_raw.serialize().unwrap());
    for (format, extension) in &raw_formats[1..] {
        let path = temp.join(format!("single-raw.{extension}"));
        assert!(export_journey_raw_data(
            path.to_string_lossy().into_owned(),
            invalid_id.clone(),
            *format,
        )
        .is_err());
        assert!(!path.exists());
    }
    for id in ids.into_iter().chain([empty_raw_id, invalid_id]) {
        delete_journey_raw_data(id).unwrap();
    }
    // Presence queries must not decode even a corrupt attachment.
    assert!(has_journeys_raw_data().unwrap());
    delete_journey_raw_data(corrupt_id).unwrap();
    assert_empty_raw(false);
}
