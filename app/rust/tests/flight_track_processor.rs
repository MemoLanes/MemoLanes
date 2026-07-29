pub mod test_utils;

use memolanes_core::flight_track_processor;
use memolanes_core::gps_processor::{Point, RawData};
use memolanes_core::{export_data, import_data};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;

fn raw_data(longitude: f64) -> RawData {
    RawData {
        point: Point {
            latitude: 0.0,
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
    for name in &[
        "CHH7867_XIAN_HANGZHOU",
        "interpolate_cross_180",
        "tokyo_hawaii",
        "TV9882-3bf27ed6",
    ] {
        const GENERATE_RESULT_GPX_FOR_INSPECTION: bool = false;

        let (loaded_data, _preprocessor) =
            import_data::load_kml(&format!("./tests/data/flight_{name}.kml")).unwrap();
        let result = flight_track_processor::process(&loaded_data).unwrap();
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
        let mut gpx = Vec::new();
        export_data::gpx::journey_vector_to_gpx_file(&result, &mut Cursor::new(&mut gpx)).unwrap();
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
