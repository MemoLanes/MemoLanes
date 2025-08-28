pub mod test_utils;

use memolanes_core::flight_track_processor::PathInterpolator;
use memolanes_core::{export_data, import_data};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::Path;

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
    let loaded_data =
        import_data::load_kml(&format!("./tests/data/interpolate/{}.kml", name)).unwrap();

    let result = PathInterpolator::interpolate(&loaded_data[0]);
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
