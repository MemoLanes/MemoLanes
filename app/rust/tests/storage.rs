pub mod test_utils;
use crate::test_utils::{draw_line1, draw_line2, draw_line3};
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

    let (raw_data_groups, _preprocessor) =
        import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx").unwrap();
    for (i, raw_data) in raw_data_groups.iter().flatten().enumerate() {
        storage.record_gps_data(
            raw_data,
            ProcessResult::Append,
            raw_data.timestamp_ms.unwrap(),
        );
        if i == 1000 {
            let _: JourneyBitmap = storage
                .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), true)
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
                .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), true)
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
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true
            )
            .unwrap(),
        default
    );
    assert_eq!(
        &storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::Flight)),
                true
            )
            .unwrap(),
        flight
    );
    let mut all = default.clone();
    all.merge(flight.clone());
    assert_eq!(
        storage
            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), true)
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

#[test]
fn delete_journey_invalidates_monthly_cache() {
    setup_storage_for_test(|storage| {
        let mut bitmap_jan = JourneyBitmap::new();
        draw_line1(&mut bitmap_jan);
        let mut bitmap_feb = JourneyBitmap::new();
        draw_line2(&mut bitmap_feb);

        let jan_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_jan.clone()),
                )
            })
            .unwrap();

        let _feb_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 2, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_feb.clone()),
                )
            })
            .unwrap();

        // Populate caches by reading
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();

        // Delete Jan journey
        storage
            .with_db_txn(|txn| txn.delete_journey(&jan_id))
            .unwrap();

        // get_latest should now return only Feb data
        let result = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        assert_eq!(result, bitmap_feb);
    });
}

#[test]
fn update_metadata_cross_month_invalidates_both() {
    setup_storage_for_test(|storage| {
        let mut bitmap = JourneyBitmap::new();
        draw_line1(&mut bitmap);

        let id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap.clone()),
                )
            })
            .unwrap();

        // Populate caches
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();

        // Move from Mar to May
        storage
            .with_db_txn(|txn| {
                txn.update_journey_metadata(
                    &id,
                    NaiveDate::from_ymd_opt(2024, 5, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                )
            })
            .unwrap();

        // Verify bitmap shows in May
        let result = storage
            .get_range_bitmap(
                NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(result, bitmap);

        // Verify bitmap NOT in Mar
        let result_mar = storage
            .get_range_bitmap(
                NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(result_mar, JourneyBitmap::new());
    });
}

#[test]
fn update_metadata_change_kind_invalidates_both() {
    setup_storage_for_test(|storage| {
        let mut bitmap = JourneyBitmap::new();
        draw_line1(&mut bitmap);

        let id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap.clone()),
                )
            })
            .unwrap();

        // Populate caches
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();

        // Change kind from DefaultKind to Flight
        storage
            .with_db_txn(|txn| {
                txn.update_journey_metadata(
                    &id,
                    NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::Flight,
                )
            })
            .unwrap();

        // Default should be empty
        let default_result = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        assert_eq!(default_result, JourneyBitmap::new());

        // Flight should have the bitmap
        let flight_result = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::Flight)),
                true,
            )
            .unwrap();
        assert_eq!(flight_result, bitmap);
    });
}

#[test]
fn get_range_bitmap_works() {
    setup_storage_for_test(|storage| {
        let mut bitmap_jan = JourneyBitmap::new();
        draw_line1(&mut bitmap_jan);
        let mut bitmap_mar = JourneyBitmap::new();
        draw_line2(&mut bitmap_mar);
        let mut bitmap_flight = JourneyBitmap::new();
        draw_line3(&mut bitmap_flight);

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_jan.clone()),
                )
            })
            .unwrap();

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_mar.clone()),
                )
            })
            .unwrap();

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2024, 1, 20).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::Flight,
                    None,
                    JourneyData::Bitmap(bitmap_flight.clone()),
                )
            })
            .unwrap();

        // Range Jan-Mar for DefaultKind
        let result = storage
            .get_range_bitmap(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 3, 31).unwrap(),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();

        let mut expected = bitmap_jan.clone();
        expected.merge(bitmap_mar.clone());
        assert_eq!(result, expected);

        // Range Jan only for Flight
        let flight_result = storage
            .get_range_bitmap(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
                Some(&JourneyKind::Flight),
            )
            .unwrap();
        assert_eq!(flight_result, bitmap_flight);

        // All kinds
        let all_result = storage
            .get_range_bitmap(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
                None,
            )
            .unwrap();
        let mut expected_all = bitmap_jan;
        expected_all.merge(bitmap_flight);
        assert_eq!(all_result, expected_all);
    });
}

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn insert_cache_delete_requery_range() {
    setup_storage_for_test(|storage| {
        let mut bitmap_jan = JourneyBitmap::new();
        draw_line1(&mut bitmap_jan);
        let mut bitmap_mar = JourneyBitmap::new();
        draw_line2(&mut bitmap_mar);

        let jan_id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-01-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_jan.clone()),
                )
            })
            .unwrap();

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_mar.clone()),
                )
            })
            .unwrap();

        // Populate cache via range query
        let _ = storage
            .get_range_bitmap(
                date("2024-01-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();

        // Delete Jan
        storage
            .with_db_txn(|txn| txn.delete_journey(&jan_id))
            .unwrap();

        // Re-query Jan-Mar: should only have Mar
        let result = storage
            .get_range_bitmap(
                date("2024-01-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(result, bitmap_mar);

        // Re-query just Jan: should be empty
        let jan_result = storage
            .get_range_bitmap(
                date("2024-01-01"),
                date("2024-01-31"),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(jan_result, JourneyBitmap::new());
    });
}

#[test]
fn update_journey_data_invalidates_and_requery_correct() {
    setup_storage_for_test(|storage| {
        let mut bitmap_line1 = JourneyBitmap::new();
        draw_line1(&mut bitmap_line1);

        let id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_line1.clone()),
                )
            })
            .unwrap();

        // Populate cache
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();

        // Update data to line2
        let mut bitmap_line2 = JourneyBitmap::new();
        draw_line2(&mut bitmap_line2);
        storage
            .with_db_txn(|txn| {
                txn.update_journey_data_with_latest_postprocessor(
                    &id,
                    JourneyData::Bitmap(bitmap_line2.clone()),
                )
            })
            .unwrap();

        // Re-query: should return line2, not stale line1
        let result = storage
            .get_range_bitmap(
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(result, bitmap_line2);
    });
}

#[test]
fn multiple_inserts_in_sequence_final_state_correct() {
    setup_storage_for_test(|storage| {
        let mut bitmap1 = JourneyBitmap::new();
        draw_line1(&mut bitmap1);
        let mut bitmap2 = JourneyBitmap::new();
        draw_line2(&mut bitmap2);
        let mut bitmap3 = JourneyBitmap::new();
        draw_line3(&mut bitmap3);

        // Insert 3 journeys one-at-a-time (3 separate with_db_txn), same month+kind
        let _id1 = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-10"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap1.clone()),
                )
            })
            .unwrap();

        // After first insert: verify
        let result1 = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        assert_eq!(result1, bitmap1);

        let id2 = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap2.clone()),
                )
            })
            .unwrap();

        // After second insert: cumulative merge
        let result2 = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        let mut expected2 = bitmap1.clone();
        expected2.merge(bitmap2.clone());
        assert_eq!(result2, expected2);

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-20"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap3.clone()),
                )
            })
            .unwrap();

        // After third insert: cumulative merge of all 3
        let result3 = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        let mut expected3 = bitmap1.clone();
        expected3.merge(bitmap2.clone());
        expected3.merge(bitmap3.clone());
        assert_eq!(result3, expected3);

        // Delete middle one
        storage.with_db_txn(|txn| txn.delete_journey(&id2)).unwrap();

        // Final state = journey1 + journey3
        let final_result = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();
        let mut expected_final = bitmap1;
        expected_final.merge(bitmap3);
        assert_eq!(final_result, expected_final);
    });
}

#[test]
fn delete_all_clears_cache_and_requery_empty() {
    setup_storage_for_test(|storage| {
        let mut bitmap_jan = JourneyBitmap::new();
        draw_line1(&mut bitmap_jan);
        let mut bitmap_mar = JourneyBitmap::new();
        draw_line2(&mut bitmap_mar);

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-01-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap_jan),
                )
            })
            .unwrap();

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-15"),
                    None,
                    None,
                    None,
                    JourneyKind::Flight,
                    None,
                    JourneyData::Bitmap(bitmap_mar),
                )
            })
            .unwrap();

        // Populate caches
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), true)
            .unwrap();

        // Delete all
        storage
            .with_db_txn(|txn| txn.delete_all_journeys())
            .unwrap();

        // get_latest should return empty
        let result = storage
            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), true)
            .unwrap();
        assert_eq!(result, JourneyBitmap::new());

        // get_range should return empty
        let range_result = storage
            .get_range_bitmap(date("2024-01-01"), date("2024-12-31"), None)
            .unwrap();
        assert_eq!(range_result, JourneyBitmap::new());
    });
}

#[test]
fn update_metadata_same_month_cache_still_correct() {
    setup_storage_for_test(|storage| {
        let mut bitmap = JourneyBitmap::new();
        draw_line1(&mut bitmap);

        let id = storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    date("2024-03-15"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(bitmap.clone()),
                )
            })
            .unwrap();

        // Populate cache
        let _ = storage
            .get_latest_bitmap_for_main_map_renderer(
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                true,
            )
            .unwrap();

        // Update date within same month (Mar 15 → Mar 20), same kind
        storage
            .with_db_txn(|txn| {
                txn.update_journey_metadata(
                    &id,
                    date("2024-03-20"),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                )
            })
            .unwrap();

        // get_range for Mar should still return the bitmap
        let result = storage
            .get_range_bitmap(
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
            .unwrap();
        assert_eq!(result, bitmap);
    });
}
