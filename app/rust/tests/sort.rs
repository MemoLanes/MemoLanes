pub mod test_utils;

use std::fs;

use memolanes_core::api::api;
use tempdir::TempDir;

#[test]
fn sort() {
    let temp_dir = TempDir::new("sort-basic").unwrap();
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

    let raw_data_list = test_utils::load_raw_gpx_data_for_test();
    let mut raw_data_list_reverse = raw_data_list.clone();
    raw_data_list_reverse.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));

    api::on_location_update(raw_data_list_reverse, 1695150531000);

    // this should cover real time update
    let render_result = api::render_map_overlay(11.0, 121.39, 31.3146, 121.55, 31.18).unwrap();
    test_utils::assert_image(
        &render_result.data,
        "sort_basic",
        "fb9a5c4ed17780375b1bd409423e2e5e4ec65bd0",
    );
}
