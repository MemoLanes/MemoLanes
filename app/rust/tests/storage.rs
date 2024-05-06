pub mod test_utils;
use memolanes_core::{
    gps_processor::{ProcessResult, RawData},
    journey_bitmap::JourneyBitmap,
    storage::Storage,
};
use std::fs;
use tempdir::TempDir;

#[test]
fn storage_for_main_map_renderer() {
    let temp_dir = TempDir::new("storage_for_main_map_renderer-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let sub_folder = |sub| {
        let path = temp_dir.path().join(sub);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };

    let storage = Storage::init(
        sub_folder("temp/"),
        sub_folder("doc/"),
        sub_folder("support/"),
        sub_folder("cache/"),
    );

    let raw_data_list = test_utils::load_raw_gpx_data_for_test();

    for (i, raw_data) in raw_data_list.iter().enumerate() {
        storage.record_gps_data(
            &RawData {
                latitude: raw_data.latitude,
                longitude: raw_data.longitude,
                timestamp_ms: raw_data.timestamp_ms,
                accuracy: raw_data.accuracy,
                altitude: raw_data.altitude,
                speed: raw_data.speed,
            },
            ProcessResult::Append,
            raw_data.timestamp_ms.unwrap(),
        );
        if i == 1000 {
            let _: JourneyBitmap = storage.get_latest_bitmap_for_main_map_renderer().unwrap();
        } else if i == 1005 {
            assert!(!storage.main_map_renderer_need_to_reload());
        } else if i == 1010 {
            let _: bool = storage
                .with_db_txn(|txn| txn.finalize_ongoing_journey())
                .unwrap();
        } else if i == 1020 {
            assert!(storage.main_map_renderer_need_to_reload());
            let _: JourneyBitmap = storage.get_latest_bitmap_for_main_map_renderer().unwrap();
        } else if i == 1025 {
            assert!(!storage.main_map_renderer_need_to_reload());
        }
    }
}
