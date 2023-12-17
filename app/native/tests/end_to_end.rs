pub mod test_utils;

use std::fs;

use native::api;
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

    for (i, raw_data) in test_utils::load_raw_gpx_data_for_test().iter().enumerate() {
        api::on_location_update(
            raw_data.latitude,
            raw_data.longitude,
            raw_data.timestamp_ms,
            raw_data.accuracy,
            raw_data.altitude,
            raw_data.speed,
        );

        if i == 1000 {
            api::finalize_ongoing_journey();
        } else if i == 2000 {
            // we have both ongoing journy and finalized journy at this point
            let render_result =
                api::render_map_overlay(11.0, 121.39, 31.3146, 121.55, 31.18).unwrap();
            test_utils::assert_image(
                &render_result.data.0,
                "end_to_end_basic_0",
                "2b7cd2dc29fffbe036e208b04c9bc46298a7e12e",
            );
        }
    }

    // this should cover real time update
    let render_result = api::render_map_overlay(11.0, 121.39, 31.3146, 121.55, 31.18).unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "end_to_end_basic_1",
        "fb9a5c4ed17780375b1bd409423e2e5e4ec65bd0",
    );
}
