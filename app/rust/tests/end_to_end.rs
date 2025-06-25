pub mod test_utils;
use memolanes_core::{api::api, gps_processor::RawData, import_data};
use rand::prelude::SliceRandom;
use std::fs;
use tempdir::TempDir;

#[test]
fn basic() {
    let temp_dir = TempDir::new("end_to_end-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let sub_folder = |sub| {
        let path = temp_dir.path().join(sub);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };

    api::init(
        sub_folder("temp/"),
        sub_folder("doc/"),
        sub_folder("support/"),
        sub_folder("cache/"),
    );

    let mut raw_data_list: Vec<RawData> =
        import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx")
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
    let (first_elements, remaining_elements) = raw_data_list.split_at_mut(2000);
    let map_renderer = api::for_testing::get_main_map_renderer();

    assert!(!api::has_ongoing_journey().unwrap());
    for (i, raw_data) in first_elements.iter().enumerate() {
        api::on_location_update(vec![raw_data.clone()], raw_data.timestamp_ms.unwrap());
        if i == 1000 {
            assert!(api::has_ongoing_journey().unwrap());
            assert!(api::finalize_ongoing_journey().unwrap());
        }
    }

    // we have both ongoing journey and finalized journey at this point
    {
        let map_renderer = map_renderer.lock().unwrap();
        let render_result =
            test_utils::render_map_overlay(&map_renderer, 11, 121.39, 31.3146, 121.55, 31.18);
        drop(map_renderer);
        test_utils::verify_image("end_to_end_basic_0", &render_result.data);
    }

    assert!(api::has_ongoing_journey().unwrap());
    assert!(api::finalize_ongoing_journey().unwrap());
    assert!(!api::has_ongoing_journey().unwrap());
    assert!(!api::finalize_ongoing_journey().unwrap());

    remaining_elements.shuffle(&mut rand::rng());
    api::on_location_update(remaining_elements.to_vec(), 1695150531000);

    {
        let map_renderer = map_renderer.lock().unwrap();
        let render_result =
            test_utils::render_map_overlay(&map_renderer, 11, 121.39, 31.3146, 121.55, 31.18);
        drop(map_renderer);
        test_utils::verify_image("end_to_end_basic_1", &render_result.data);
    }

    // try export logs
    api::export_logs("./tests/for_inspection/end_to_end_basic-logs.zip".to_string()).unwrap();
}
