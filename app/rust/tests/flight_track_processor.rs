pub mod test_utils;

use memolanes_core::flight_track_processor;
use memolanes_core::gps::{Point, RawGPSPoint};
use memolanes_core::{export_data, import_data};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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
fn run_tests() {
    let temp = tempdir::TempDir::new("flight-track-export").unwrap();
    for name in &[
        "CHH7867_XIAN_HANGZHOU",
        "interpolate_cross_180",
        "tokyo_hawaii",
        "TV9882-3bf27ed6",
    ] {
        const GENERATE_RESULT_GPX_FOR_INSPECTION: bool = false;

        let (loaded_data, _preprocessor) =
            import_data::kml::load_kml(&format!("./tests/data/flight_{name}.kml")).unwrap();
        let result = flight_track_processor::process(&loaded_data).unwrap();
        assert_source_points_and_spherical_interpolation(&loaded_data, &result);
        let input_point_count: usize = loaded_data.iter().map(Vec::len).sum();
        let output_point_count: usize = result
            .track_segments
            .iter()
            .map(|segment| segment.track_points.len())
            .sum();
        assert!(
            output_point_count >= input_point_count,
            "{name}: flight track interpolation removed points ({input_point_count} -> {output_point_count})"
        );
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
        let gpx = fs::read(path).unwrap();
        verify_gpx(name, &gpx);
        if GENERATE_RESULT_GPX_FOR_INSPECTION {
            let mut file = File::create(format!(
                "./tests/for_inspection/flight_track_processor_{name}.gpx"
            ))
            .unwrap();
            file.write_all(&gpx).unwrap();
        }
    }
}

fn verify_gpx(name: &str, gpx_data: &[u8]) {
    let hash_table_path = "tests/gpx_hashes.lock";
    let mut hash_table: BTreeMap<String, String> = if Path::new(hash_table_path).exists() {
        let hash_table_content =
            fs::read_to_string(hash_table_path).expect("Failed to read hash table file");
        serde_json::from_str(&hash_table_content).unwrap_or_else(|_| BTreeMap::new())
    } else {
        BTreeMap::new()
    };

    // Calculate hash of the gpx file
    let mut hasher = Sha256::new();
    hasher.update(gpx_data);
    let current_hash = hex::encode(hasher.finalize());

    if let Some(stored_hash) = hash_table.get(name) {
        // Entry exists, compare hashes
        assert_eq!(
            &current_hash, stored_hash,
            "Gpx file hash mismatch for {name}. Expected: {stored_hash}, Got: {current_hash}. If you have updated the gpx file, please delete the gpx_hashes.lock file and re-run the tests."
        );
        println!("Verified gpx file hash for: {name}");
    } else {
        // No entry exists, add new entry
        hash_table.insert(name.to_string(), current_hash.clone());
        let hash_table_content =
            serde_json::to_string_pretty(&hash_table).expect("Failed to serialize hash table");
        fs::write(hash_table_path, hash_table_content).expect("Failed to write hash table file");
        println!("Added new hash entry for: {name}");
    }
}

#[test]
fn sparse_arctic_flight_follows_the_shortest_arc_below_the_bitmap_seam() {
    // The Greenland/Siberia coverage gap from CX841. Neither endpoint, nor
    // the true arc, enters the >85.05-degree bitmap cap.
    for sign in [1.0, -1.0] {
        let source = vec![
            raw_data_at(-54.15144, sign * 74.692955),
            raw_data_at(82.201324, sign * 72.745468),
        ];
        let result = flight_track_processor::process(std::slice::from_ref(&source)).unwrap();
        assert_source_points_and_spherical_interpolation(std::slice::from_ref(&source), &result);
        let points = &result.track_segments[0].track_points;
        let maximum_latitude = points
            .iter()
            .map(|p| sign * p.latitude)
            .fold(-90.0, f64::max);
        assert!(
            maximum_latitude > 83.82 && maximum_latitude < 83.84,
            "the short Arctic arc reaches 83.83 degrees, got {maximum_latitude}"
        );
    }
}

fn assert_source_points_and_spherical_interpolation(
    raw: &[Vec<RawGPSPoint>],
    result: &memolanes_core::journey_vector::JourneyVector,
) {
    // Flatten only for checking the spherical path: paired ±180° endpoints
    // occupy the same physical location even though planar consumers need a break.
    let points: Vec<_> = result
        .track_segments
        .iter()
        .flat_map(|segment| &segment.track_points)
        .collect();
    let mut index = 0;
    for source in raw.iter().filter(|s| !s.is_empty()) {
        let matches = |point: &memolanes_core::journey_vector::TrackPoint, source: &RawGPSPoint| {
            point.latitude == source.point.latitude && point.longitude == source.point.longitude
        };
        assert!(matches(points[index], &source[0]));
        for pair in source.windows(2) {
            let next = index
                + 1
                + points[index + 1..]
                    .iter()
                    .position(|p| matches(p, &pair[1]))
                    .expect("every source point must survive in order");
            let a = pair[0].point.to_cartesian();
            let b = pair[1].point.to_cartesian();
            let n = (
                a.1 * b.2 - a.2 * b.1,
                a.2 * b.0 - a.0 * b.2,
                a.0 * b.1 - a.1 * b.0,
            );
            let norm = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt();
            let mut length = 0.0;
            for point in &points[index + 1..next] {
                let p = Point {
                    longitude: point.longitude,
                    latitude: point.latitude,
                }
                .to_cartesian();
                if norm > 1e-12 {
                    let deviation =
                        ((n.0 * p.0 + n.1 * p.1 + n.2 * p.2) / norm).abs().asin() * 6_371_000.0;
                    assert!(
                        deviation < 0.2,
                        "interpolated point deviates by {deviation} metres from the spherical path"
                    );
                }
            }
            for segment in points[index..=next].windows(2) {
                let a = Point {
                    longitude: segment[0].longitude,
                    latitude: segment[0].latitude,
                };
                let b = Point {
                    longitude: segment[1].longitude,
                    latitude: segment[1].latitude,
                };
                let step = a.haversine_distance(&b);
                assert!(step <= 1_000.2, "interpolation left a {step}-metre gap");
                length += step;
            }
            assert!(
                (length - pair[0].point.haversine_distance(&pair[1].point)).abs() < 1.0,
                "flight interpolation takes a detour"
            );
            index = next;
        }
        index += 1;
    }
    assert_eq!(index, points.len());
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
        assert_source_points_and_spherical_interpolation(std::slice::from_ref(&source), &result);
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
    assert_source_points_and_spherical_interpolation(&raw, &result);
    assert_eq!(result.track_segments.len(), 2);
}

#[test]
fn arctic_kml_import_and_postprocessing_rasterize_the_spherical_route() {
    use journey_kernel::tile_range::decode_tile_range_response;
    use memolanes_core::{
        gps_processor::GpsPostprocessor, journey_bitmap::JourneyBitmap, renderer::MapRenderer,
        utils::lng_lat_to_tile_x_y,
    };
    let (raw, _) = import_data::kml::load_kml("tests/data/flight_arctic_gap.kml").unwrap();
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0].len(), 6);
    let result = flight_track_processor::process(&raw).unwrap();
    assert_source_points_and_spherical_interpolation(&raw, &result);
    let stored = GpsPostprocessor::process(result);
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(&stored);
    assert!(
        !bitmap.has_polar_data(),
        "this route stays below the bitmap cap while still crossing the Arctic"
    );
    let mut renderer = MapRenderer::new(bitmap);
    let mut covered = |lng: f64, lat: f64| {
        let (x, y) = lng_lat_to_tile_x_y(lng, lat, 14);
        let (px, py) = lng_lat_to_tile_x_y(lng, lat, 22);
        let response = renderer
            .get_tile_range_response(x.into(), y.into(), 14, 1, 1, 8)
            .unwrap();
        decode_tile_range_response(&response)
            .unwrap()
            .first()
            .is_some_and(|(_, _, bitmap)| {
                bitmap.get(px.rem_euclid(256) as usize, py.rem_euclid(256) as usize)
            })
    };
    assert!(
        covered(22.293726396749058, 83.73896566386834),
        "the stored bitmap must pass through the spherical midpoint"
    );
    assert!(
        !covered(22.293726396749058, 73.7),
        "longitude/latitude interpolation must not leave the old southward detour"
    );
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
            assert_source_points_and_spherical_interpolation(
                std::slice::from_ref(&source),
                &result,
            );
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
