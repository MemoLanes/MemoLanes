pub mod test_utils;
use memolanes_core::{api::api, gps_processor::RawData, import_data};
use rand::prelude::SliceRandom;
use rand::thread_rng;
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
    let mut map_renderer_proxy = api::get_map_renderer_proxy_for_main_map();

    assert!(!api::has_ongoing_journey().unwrap());
    for (i, raw_data) in first_elements.iter().enumerate() {
        api::on_location_update(vec![raw_data.clone()], raw_data.timestamp_ms.unwrap());
        if i == 1000 {
            assert!(api::has_ongoing_journey().unwrap());
            let _: bool = api::finalize_ongoing_journey().unwrap();
        } else if i == 2000 {
            // we have both ongoing journey and finalized journey at this point
            // TODO: check in webview based test
            // let render_result = map_renderer_proxy
            //     .render_map_overlay(11.0, 121.39, 31.3146, 121.55, 31.18)
            //     .unwrap();
            // test_utils::verify_image("end_to_end_basic_0", &render_result.data);
        }
    }

    remaining_elements.shuffle(&mut thread_rng());
    api::on_location_update(remaining_elements.to_vec(), 1695150531000);

    // TODO: check in webview based test
    // this should cover real time update
    // let render_result = map_renderer_proxy
    //     .render_map_overlay(11.0, 121.39, 31.3146, 121.55, 31.18)
    //     .unwrap();
    // test_utils::verify_image("end_to_end_basic_1", &render_result.data);

    // try export logs
    api::export_logs("./tests/for_inspection/end_to_end_basic-logs.zip".to_string()).unwrap();
}
