pub mod test_utils;
use crate::test_utils::{draw_line1, draw_line2};
use chrono::NaiveDate;
use memolanes_core::{
    cache_db::LayerKind, gps_processor::ProcessResult, import_data, journey_bitmap::JourneyBitmap,
    journey_data::JourneyData, journey_header::JourneyKind, storage::Storage,
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

fn setup_storage_for_test<F>(f: F)
where
    F: FnOnce(Storage),
{
    let temp_dir = TempDir::new("test_storage").unwrap();
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
    f(storage);
}

fn assert_cache(storage: &Storage, default: &JourneyBitmap, flight: &JourneyBitmap) {
    assert_eq!(
        &storage
            .get_latest_bitmap_for_main_map_renderer(&LayerKind::JourneyKind(
                JourneyKind::DefaultKind
            ))
            .unwrap(),
        default
    );
    assert_eq!(
        &storage
            .get_latest_bitmap_for_main_map_renderer(&LayerKind::JourneyKind(JourneyKind::Flight))
            .unwrap(),
        flight
    );
    let mut all = default.clone();
    all.merge(flight.clone());
    assert_eq!(
        storage
            .get_latest_bitmap_for_main_map_renderer(&LayerKind::All)
            .unwrap(),
        all
    );
}

#[test]
fn increment_journey_and_verify_cache_same_kind() {
    setup_storage_for_test(|storage| {
        let empty_bitmap = JourneyBitmap::new();

        let mut journey_bitmap1 = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap1);
        let mut journey_bitmap2 = JourneyBitmap::new();
        draw_line2(&mut journey_bitmap2);

        let mut total_bitmap = JourneyBitmap::new();
        total_bitmap.merge(journey_bitmap1.clone());
        total_bitmap.merge(journey_bitmap2.clone());

        assert_cache(&storage, &empty_bitmap, &empty_bitmap);

        let journey1_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(journey_bitmap1.clone()),
                )
            })
            .unwrap();

        assert_cache(&storage, &journey_bitmap1, &empty_bitmap);

        let journey2_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(journey_bitmap2.clone()),
                )
            })
            .unwrap();

        assert_cache(&storage, &total_bitmap, &empty_bitmap);

        storage
            .with_db_txn(|txn| txn.delete_journey(&journey1_id))
            .unwrap();
        assert_cache(&storage, &journey_bitmap2, &empty_bitmap);
        storage
            .with_db_txn(|txn| txn.delete_journey(&journey2_id))
            .unwrap();
        assert_cache(&storage, &empty_bitmap, &empty_bitmap);
    });
}

#[test]
fn increment_journey_and_verify_cache_different_kind() {
    setup_storage_for_test(|storage| {
        let empty_bitmap = JourneyBitmap::new();

        let mut journey_bitmap1 = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap1);
        let mut journey_bitmap2 = JourneyBitmap::new();
        draw_line2(&mut journey_bitmap2);

        assert_cache(&storage, &empty_bitmap, &empty_bitmap);

        let journey1_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(journey_bitmap1.clone()),
                )
            })
            .unwrap();

        assert_cache(&storage, &journey_bitmap1, &empty_bitmap);

        let journey2_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::Flight,
                    None,
                    JourneyData::Bitmap(journey_bitmap2.clone()),
                )
            })
            .unwrap();

        assert_cache(&storage, &journey_bitmap1, &journey_bitmap2);

        storage
            .with_db_txn(|txn| txn.delete_journey(&journey1_id))
            .unwrap();
        assert_cache(&storage, &empty_bitmap, &journey_bitmap2);
        storage
            .with_db_txn(|txn| txn.delete_journey(&journey2_id))
            .unwrap();
        assert_cache(&storage, &empty_bitmap, &empty_bitmap);
    });
}
