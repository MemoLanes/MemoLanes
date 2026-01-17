pub mod test_utils;

use memolanes_core::flight_track_processor;
use memolanes_core::{export_data, import_data};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;

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
        let mut gpx = Vec::new();
        export_data::journey_vector_to_gpx_file(&result, &mut Cursor::new(&mut gpx)).unwrap();
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
    let current_hash = format!("{:x}", hasher.finalize());

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
