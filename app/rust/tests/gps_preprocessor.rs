pub mod test_utils;

use memolanes_core::gps_processor::{
    GpsPreprocessor, PathInterpolator, Point, ProcessResult, RawData,
};
use memolanes_core::{export_data, import_data};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::path::Path;

#[test]
fn first_data() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    assert!(gps_preprocessor.last_kept_point().is_none());
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );
}

#[test]
fn ignore() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(300.0),
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    assert_eq!(gps_preprocessor.preprocess(&data), ProcessResult::Ignore);
}

#[test]
fn time_difference() {
    let mut gps_preprocessor = GpsPreprocessor::new();

    gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    });

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.163856
    );
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.1639266,
            longitude: 30.271981,
        },
        timestamp_ms: Some(1697349117449),
        accuracy: Some(3.5),
        altitude: Some(20.),
        speed: Some(0.18825254),
    });
    assert_eq!(ProcessResult::Append, result);

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.1639266
    );
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163857,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1698349116449),
        accuracy: Some(3.9),
        altitude: Some(30.),
        speed: Some(0.6028665),
    });
    assert_eq!(ProcessResult::NewSegment, result);

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.163857
    );
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    });
    assert_eq!(ProcessResult::Ignore, result);
}

#[test]
fn speed() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116000),
        accuracy: None,
        altitude: None,
        speed: None,
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );

    let data = RawData {
        point: Point {
            latitude: 125.0,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349117000),
        accuracy: None,
        altitude: None,
        speed: None,
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );
}

fn run_though_test_data(name: &str) -> HashMap<ProcessResult, i32> {
    const GENERATE_RESULT_GPX_FOR_INSPECTION: bool = false;
    let mut gps_preprocessor = GpsPreprocessor::new();
    let mut counter = HashMap::new();
    let loaded_data = import_data::load_gpx(&format!("./tests/data/raw_gps_{}.gpx", name)).unwrap();
    for data in loaded_data.iter().flatten() {
        let result = gps_preprocessor.preprocess(data);
        counter.entry(result).and_modify(|c| *c += 1).or_insert(1);
    }

    if GENERATE_RESULT_GPX_FOR_INSPECTION {
        let journey_vector = import_data::journey_vector_from_raw_data(loaded_data, true).unwrap();

        let mut file = File::create(format!(
            "./tests/for_inspection/gps_preprocessor_run_though_test_data_{}.gpx",
            name
        ))
        .unwrap();
        export_data::journey_vector_to_gpx_file(&journey_vector, &mut file).unwrap();
    };

    counter
}

#[test]
fn run_though_test_data_shanghai() {
    let counter = run_though_test_data("shanghai");
    assert_eq!(counter[&ProcessResult::NewSegment], 11);
    assert_eq!(counter[&ProcessResult::Append], 2959);
    assert_eq!(counter[&ProcessResult::Ignore], 648);
}

#[test]
fn run_though_test_data_shenzhen_stationary() {
    let counter = run_though_test_data("shenzhen_stationary");
    assert_eq!(counter[&ProcessResult::NewSegment], 8);
    assert_eq!(counter[&ProcessResult::Append], 443);
    assert_eq!(counter[&ProcessResult::Ignore], 6279);
}

#[test]
fn run_though_test_data_laojunshan() {
    let counter = run_though_test_data("laojunshan");
    assert_eq!(counter[&ProcessResult::NewSegment], 2);
    assert_eq!(counter[&ProcessResult::Append], 2595);
    assert_eq!(counter[&ProcessResult::Ignore], 348);
}

#[test]
fn run_interpolate_test() {
    interpolate_test("CHH7867_XIAN_HANGZHOU");
    interpolate_test("CSN3281_TwoPoint");
    interpolate_test("interpolate");
    interpolate_test("interpolatecross180");
    interpolate_test("tokyo_hawaii");
    interpolate_test("tokyo_hawaii2");
}

fn interpolate_test(name: &str) {
    let interpolator = PathInterpolator::new();
    let loaded_data =
        import_data::load_kml(&format!("./tests/data/interpolate/{}.kml", name)).unwrap();

    let result = interpolator.interpolate(&loaded_data[0]);
    let mut file =
        File::create(format!("./tests/for_inspection/interpolate_{}.gpx", name)).unwrap();
    export_data::journey_vector_to_gpx_file(&result, &mut file).unwrap();

    match fs::read(format!("./tests/for_inspection/interpolate_{}.gpx", name)) {
        Ok(filedata) => verify_gpx(name, &filedata),
        Err(_) => {
            println!("file load error!")
        }
    }
}

fn verify_gpx(name: &str, gpx_file: &[u8]) {
    let hash_table_path = "tests/gpx_hashes.lock";
    let mut hash_table: BTreeMap<String, String> = if Path::new(hash_table_path).exists() {
        let hash_table_content =
            fs::read_to_string(hash_table_path).expect("Failed to read hash table file");
        serde_json::from_str(&hash_table_content).unwrap_or_else(|_| BTreeMap::new())
    } else {
        BTreeMap::new()
    };

    // Calculate hash of the current image
    let mut hasher = Sha256::new();
    hasher.update(gpx_file);
    let current_hash = format!("{:x}", hasher.finalize());

    if let Some(stored_hash) = hash_table.get(name) {
        // Entry exists, compare hashes
        assert_eq!(
            &current_hash, stored_hash,
            "Gpx file hash mismatch for {}. Expected: {}, Got: {}. If you have updated the gpx file, please delete the gpx_hashes.lock file and re-run the tests.",
            name, stored_hash, current_hash
        );
        println!("Verified gpx file hash for: {}", name);
    } else {
        // No entry exists, add new entry
        hash_table.insert(name.to_string(), current_hash.clone());
        let hash_table_content =
            serde_json::to_string_pretty(&hash_table).expect("Failed to serialize hash table");
        fs::write(hash_table_path, hash_table_content).expect("Failed to write hash table file");
        println!("Added new hash entry for: {}", name);
    }
}
