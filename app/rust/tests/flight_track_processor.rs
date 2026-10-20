pub mod test_utils;

use memolanes_core::flight_track_processor;
use memolanes_core::gps::{Point, RawGPSPoint};
use memolanes_core::{export_data, import_data};
use std::fs::File;

fn raw_data(longitude: f64) -> RawGPSPoint {
    raw_data_at(longitude, 0.0)
}

fn raw_data_at(longitude: f64, latitude: f64) -> RawGPSPoint {
    RawGPSPoint {
        point: Point {
            latitude,
            longitude,
        },
        timestamp_ms: None,
        accuracy: None,
        altitude: None,
        speed: None,
    }
}

#[test]
fn preserves_dense_source_points() {
    let source = vec![
        raw_data(0.0),
        raw_data(0.001),
        raw_data(0.002),
        raw_data(0.003),
    ];

    let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
    let track_points = &result.track_segments[0].track_points;

    assert_eq!(track_points.len(), source.len());
    for (result, source) in track_points.iter().zip(source) {
        assert_eq!(result.latitude, source.point.latitude);
        assert_eq!(result.longitude, source.point.longitude);
    }
}

#[test]
fn fills_gaps_without_replacing_source_points() {
    let source = vec![raw_data(0.0), raw_data(0.005), raw_data(0.02)];

    let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
    let track_points = &result.track_segments[0].track_points;

    assert!(track_points.len() > source.len());
    assert_eq!(track_points[0].longitude, source[0].point.longitude);
    assert_eq!(track_points[1].longitude, source[1].point.longitude);
    assert_eq!(
        track_points.last().unwrap().longitude,
        source.last().unwrap().point.longitude
    );
}

#[test]
fn preserves_single_and_repeated_source_points() {
    let single = vec![raw_data(1.0)];
    let single_result = flight_track_processor::process(&[single]).unwrap();
    let single_track_points = &single_result.track_segments[0].track_points;
    assert_eq!(single_track_points.len(), 1);
    assert_eq!(single_track_points[0].longitude, 1.0);

    let repeated = vec![raw_data(0.0), raw_data(0.0), raw_data(0.02)];
    let repeated_result = flight_track_processor::process(std::slice::from_ref(&repeated)).unwrap();
    let repeated_track_points = &repeated_result.track_segments[0].track_points;
    assert!(repeated_track_points.len() >= repeated.len());
    assert_eq!(repeated_track_points[0].longitude, 0.0);
    assert_eq!(repeated_track_points[1].longitude, 0.0);
    assert!(repeated_track_points
        .iter()
        .all(|point| point.latitude.is_finite() && point.longitude.is_finite()));
}

#[test]
fn real_flights_preserve_observations_and_roundtrip_through_gpx() {
    let temp = tempdir::TempDir::new("flight-track-export").unwrap();
    for name in &[
        "CHH7867_XIAN_HANGZHOU",
        "interpolate_cross_180",
        "tokyo_hawaii",
        "TV9882-3bf27ed6",
        "arctic_gap",
    ] {
        let (loaded_data, _preprocessor) =
            import_data::kml::load_kml(&format!("./tests/data/flight_{name}.kml")).unwrap();
        let result = flight_track_processor::process(&loaded_data).unwrap();
        assert_preserved_points_and_spacing(&loaded_data, &result);
        let path = temp.path().join(format!("{name}.gpx"));
        export_data::gpx::journeys_to_gpx_file(
            &[test_utils::vector_export(result.clone())],
            &mut File::create(&path).unwrap(),
        )
        .unwrap();
        let segments = import_data::gpx::load_gpx(path.to_str().unwrap())
            .unwrap()
            .0;
        assert_eq!(segments.len(), result.track_segments.len());
        for (exported, source) in segments.iter().zip(&result.track_segments) {
            assert_eq!(exported.len(), source.track_points.len());
            for (exported, source) in exported.iter().zip(&source.track_points) {
                assert_eq!(exported.point.latitude, source.latitude);
                assert!((-180.0..180.0).contains(&exported.point.longitude));
                // +180 endpoints must stay on the eastern side, with only
                // floating-point rounding needed for GPX's exclusive bound.
                assert!((exported.point.longitude - source.longitude).abs() < 1e-12);
            }
        }
    }
}

fn assert_preserved_points_and_spacing(
    raw: &[Vec<RawGPSPoint>],
    result: &memolanes_core::journey_vector::JourneyVector,
) {
    let mut output = result
        .track_segments
        .iter()
        .flat_map(|segment| &segment.track_points);
    for source in raw.iter().flatten() {
        assert!(
            output.any(|point| point.latitude == source.point.latitude
                && point.longitude == source.point.longitude),
            "every original observation, including duplicates, must survive in order: {:?}",
            source.point,
        );
    }
    for segment in &result.track_segments {
        for pair in segment.track_points.windows(2) {
            let a = Point {
                longitude: pair[0].longitude,
                latitude: pair[0].latitude,
            };
            let b = Point {
                longitude: pair[1].longitude,
                latitude: pair[1].latitude,
            };
            let step = a.haversine_distance(&b);
            // Generated coordinates are rounded to six decimal places.
            assert!(step <= 1_000.2, "interpolation left a {step}-metre gap");
        }
    }
}

#[test]
fn dateline_and_pole_crossings_are_continuous_on_the_sphere() {
    for source in [
        vec![raw_data_at(170.0, 88.0), raw_data_at(-170.0, 88.2)],
        vec![raw_data_at(179.9, 70.0), raw_data_at(-179.9, 70.0)],
        vec![raw_data_at(0.0, 89.0), raw_data_at(180.0, 89.0)],
        vec![raw_data_at(0.0, -89.0), raw_data_at(180.0, -89.0)],
        vec![raw_data_at(0.0, 0.0), raw_data_at(180.0, 0.0)],
    ] {
        let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
        assert_preserved_points_and_spacing(std::slice::from_ref(&source), &result);
        assert!(result
            .track_segments
            .iter()
            .flat_map(|s| &s.track_points)
            .all(|p| p.latitude.is_finite()
                && p.longitude.is_finite()
                && p.latitude.abs() <= 90.0));
    }
}

#[test]
fn spherical_interpolation_keeps_source_segment_breaks() {
    assert!(flight_track_processor::process(&[vec![]]).is_none());
    let raw = vec![
        vec![],
        vec![raw_data(0.0), raw_data(0.02)],
        vec![raw_data(100.0), raw_data(100.02)],
    ];
    let result = flight_track_processor::process(&raw).unwrap();
    assert_preserved_points_and_spacing(&raw, &result);
    assert_eq!(result.track_segments.len(), 2);
}

#[test]
fn dateline_splits_preserve_source_points_and_share_spherical_endpoints() {
    for (start, end) in [
        ((179.99, 0.0), (-179.99, 0.0)),
        ((-179.999, 70.0), (179.999, 70.001)),
        ((170.0, 88.0), (-170.0, 88.2)),
        ((180.0, 10.0), (-179.99, 10.01)),
        ((179.99, -10.0), (-180.0, -10.01)),
    ] {
        for (start, end) in [(start, end), (end, start)] {
            let source = vec![raw_data_at(start.0, start.1), raw_data_at(end.0, end.1)];
            let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
            assert_eq!(result.track_segments.len(), 2);
            assert_preserved_points_and_spacing(std::slice::from_ref(&source), &result);
            let before = result.track_segments[0].track_points.last().unwrap();
            let after = &result.track_segments[1].track_points[0];
            assert_eq!(before.longitude.abs(), 180.0);
            assert_eq!(after.longitude, -before.longitude);
            assert_eq!(before.latitude, after.latitude);
            for segment in &result.track_segments {
                assert!(segment
                    .track_points
                    .windows(2)
                    .all(|pair| { (pair[1].longitude - pair[0].longitude).abs() <= 180.0 }));
            }
        }
    }
}

#[test]
fn spherical_smoothing_handles_repeated_and_antipodal_observations() {
    for source in [
        vec![
            raw_data_at(0.0, 0.0),
            raw_data_at(180.0, 0.0),
            raw_data_at(179.0, 1.0),
        ],
        vec![
            raw_data_at(0.0, 0.0),
            raw_data_at(1.0, 1.0),
            raw_data_at(1.0, 1.0),
            raw_data_at(2.0, 1.0),
        ],
    ] {
        let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
        assert_preserved_points_and_spacing(std::slice::from_ref(&source), &result);
        assert!(result
            .track_segments
            .iter()
            .flat_map(|segment| &segment.track_points)
            .all(|point| point.latitude.is_finite()
                && point.longitude.is_finite()
                && point.latitude.abs() <= 90.0));
    }
}
