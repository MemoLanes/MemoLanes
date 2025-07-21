pub mod test_utils;
use crate::test_utils::{draw_line1, draw_line2};
use chrono::{NaiveDate, Utc};
use memolanes_core::{
    cache_db::LayerKind, gps_processor::ProcessResult, import_data, journey_bitmap::JourneyBitmap,
    journey_data::JourneyData, journey_header::JourneyHeader, journey_header::JourneyKind,
    journey_header::JourneyType, storage::Storage,
};
use std::fs;
use tempdir::TempDir;
use uuid::Uuid;

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

#[test]
fn increment_journey_and_verify_cache() {
    let temp_dir = TempDir::new("test_incremental_add_journey").unwrap();
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

    /* create two dummy header with JourneyKind::DefaultKind type  */
    let header1 = JourneyHeader {
        id: Uuid::new_v4().to_string(),
        revision: "rev1".to_string(),
        journey_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: Utc::now(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Bitmap,
        journey_kind: JourneyKind::DefaultKind,
        note: None,
        postprocessor_algo: None,
    };

    let header2 = JourneyHeader {
        id: Uuid::new_v4().to_string(),
        revision: "rev2".to_string(),
        journey_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: Utc::now(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Bitmap,
        journey_kind: JourneyKind::DefaultKind,
        note: None,
        postprocessor_algo: None,
    };

    let mut journey_bitmap1 = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap1);
    let mut journey_bitmap2 = JourneyBitmap::new();
    draw_line2(&mut journey_bitmap2);

    let mut expected_bitmap = JourneyBitmap::new();
    draw_line1(&mut expected_bitmap);
    draw_line2(&mut expected_bitmap);

    storage
        .with_db_txn(|txn| {
            txn.insert_journey(header1, JourneyData::Bitmap(journey_bitmap1.clone()))
        })
        .unwrap();

    /* update empty DefaultKind cache */
    let _cache_bitmap = storage
        .get_latest_bitmap_for_main_map_renderer(&LayerKind::JounreyKind(JourneyKind::DefaultKind))
        .unwrap();

    storage
        .with_db_txn(|txn| {
            txn.insert_journey(header2, JourneyData::Bitmap(journey_bitmap2.clone()))
        })
        .unwrap();

    /* get current DefaultKind cache updated by with_db_txn */
    let cache_bitmap = storage
        .get_latest_bitmap_for_main_map_renderer(&LayerKind::JounreyKind(JourneyKind::DefaultKind))
        .unwrap();

    assert_eq!(cache_bitmap, expected_bitmap);
}

#[test]
fn delete_journey_and_verify_cache() {
    let temp_dir = TempDir::new("test_delete_journey").unwrap();
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

    /* create dummy header */
    let header1 = JourneyHeader {
        id: Uuid::new_v4().to_string(),
        revision: "rev1".to_string(),
        journey_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        created_at: Utc::now(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Bitmap,
        journey_kind: JourneyKind::Flight,
        note: None,
        postprocessor_algo: None,
    };

    let mut journey_bitmap1 = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap1);

    storage
        .with_db_txn(|txn| {
            txn.insert_journey(
                header1.clone(),
                JourneyData::Bitmap(journey_bitmap1.clone()),
            )
        })
        .unwrap();

    /* update empty Flight cache */
    let _cache_bitmap = storage
        .get_latest_bitmap_for_main_map_renderer(&LayerKind::JounreyKind(JourneyKind::Flight))
        .unwrap();

    /* delete journey to CompleteRebuilt */
    storage
        .with_db_txn(|txn| txn.delete_journey(header1.id.as_str()))
        .unwrap();

    let expected_bitmap = JourneyBitmap::new();
    let cache_bitmap = storage
        .get_latest_bitmap_for_main_map_renderer(&LayerKind::JounreyKind(JourneyKind::Flight))
        .unwrap();
    assert_eq!(cache_bitmap, expected_bitmap);
}
