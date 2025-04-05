pub mod test_utils;
use memolanes_core::{
    cache_db::LayerKind, gps_processor::ProcessResult, import_data, journey_bitmap::JourneyBitmap,
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

    for (i, raw_data) in import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx")
        .unwrap()
        .iter()
        .flatten()
        .enumerate()
    {
        storage.record_gps_data(
            raw_data,
            ProcessResult::Append,
            raw_data.timestamp_ms.unwrap(),
        );
        if i == 1000 {
            let _: JourneyBitmap = storage
                .get_latest_bitmap_for_main_map_renderer(&LayerKind::All)
                .unwrap();
        } else if i == 1005 {
            // TODO: reimplement the assert under new api
            // assert!(!storage.main_map_renderer_need_to_reload());
        } else if i == 1010 {
            let _: bool = storage
                .with_db_txn(|txn| txn.finalize_ongoing_journey())
                .unwrap();
        } else if i == 1020 {
            // assert!(storage.main_map_renderer_need_to_reload());
            let _: JourneyBitmap = storage
                .get_latest_bitmap_for_main_map_renderer(&LayerKind::All)
                .unwrap();
        } else if i == 1025 {
            // assert!(!storage.main_map_renderer_need_to_reload());
        }
    }
}
