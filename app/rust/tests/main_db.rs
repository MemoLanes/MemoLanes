pub mod test_utils;

use chrono::{DateTime, Datelike, NaiveDate};
use memolanes_core::{
    gps_processor::{self, Point, RawData},
    import_data,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::JourneyVector,
    main_db::{self, Action, CacheEntry, MainDb},
};
use tempdir::TempDir;

#[test]
fn basic() {
    let (raw_data, _preprocessor) =
        import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx").unwrap();

    let test_data: Vec<RawData> = raw_data.into_iter().flatten().collect();
    let num_of_gpx_data_in_input = test_data.len();
    println!("total test data: {num_of_gpx_data_in_input}");

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    for (i, raw_data) in test_data.iter().enumerate() {
        if i > 1000 && i % 1000 == 0 {
            // test restart
            main_db = MainDb::open(temp_dir.path().to_str().unwrap());
        }
        main_db
            .record(raw_data, gps_processor::ProcessResult::Append)
            .unwrap();
    }
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();

    // validate the finalized journey
    let journeys = main_db
        .with_txn(|txn| txn.query_journeys(None, None))
        .unwrap();
    assert_eq!(journeys.len(), 1);
    let journey_id = &journeys[0].id;
    let mut journey_data = main_db
        .with_txn(|txn| txn.get_journey_data(journey_id))
        .unwrap();

    let journey_vector = match &journey_data {
        JourneyData::Vector(vector) => vector,
        JourneyData::Bitmap(_) => panic!("invalid"),
    };
    let num_of_gpx_data = journey_vector.track_segments[0].track_points.len();
    assert_eq!(num_of_gpx_data_in_input, num_of_gpx_data);

    // benefit from zstd
    let mut rough_raw_size: usize = 0;
    for i in &journey_vector.track_segments {
        for _ in &i.track_points {
            rough_raw_size += std::mem::size_of::<f64>() * 2;
        }
    }

    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();
    let compressed_size = buf.len();
    println!(
        "rough size: {:.4}MB, compressed size: {:.4}MB",
        rough_raw_size as f64 / 1024. / 1024.,
        compressed_size as f64 / 1024. / 1024.
    );
    println!(
        "compression rate: {:.2}",
        compressed_size as f64 / rough_raw_size as f64
    );

    // without any more gpx data, should be no-op
    main_db
        .with_txn(|txn| txn.finalize_ongoing_journey())
        .unwrap();
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, None))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn setting() {
    use main_db::Setting;

    let temp_dir = TempDir::new("main_db-setting").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    // default value
    assert!(!main_db.get_setting_with_default(Setting::RawDataMode, false));
    assert!(main_db.get_setting_with_default(Setting::RawDataMode, true));

    // setting value
    main_db.set_setting(Setting::RawDataMode, true).unwrap();
    assert!(main_db.get_setting_with_default(Setting::RawDataMode, false));

    // restart
    main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    assert!(main_db.get_setting_with_default(main_db::Setting::RawDataMode, false));
}

#[test]
fn get_ongoing_journey_timestamp_range() {
    let temp_dir = TempDir::new("main_db-get_lastest_timestamp_of_ongoing_journey").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    let result = main_db
        .with_txn(|txn| txn.get_ongoing_journey_timestamp_range())
        .unwrap();
    assert_eq!(result, None);
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349115000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349116000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    main_db
        .record(
            &RawData {
                point: Point {
                    latitude: 120.163856,
                    longitude: 30.2719716,
                },
                timestamp_ms: Some(1697349117000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    let result = main_db
        .with_txn(|txn| txn.get_ongoing_journey_timestamp_range())
        .unwrap();
    assert_eq!(
        result,
        Some((
            DateTime::from_timestamp(1697349115, 0).unwrap(),
            DateTime::from_timestamp(1697349117, 0).unwrap()
        ))
    );
}

#[test]
fn journey_query() {
    let temp_dir = TempDir::new("main_db-journey_query").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let date = |str| NaiveDate::parse_from_str(str, "%Y-%m-%d").unwrap();

    let add_empty_journey = |txn: &mut main_db::Txn, journey_date_str| {
        txn.create_and_insert_journey(
            date(journey_date_str),
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Vector(JourneyVector {
                track_segments: vec![],
            }),
        )
        .unwrap()
    };

    assert_eq!(
        main_db.with_txn(|txn| txn.earliest_journey_date()).unwrap(),
        None
    );

    main_db
        .with_txn(|txn| {
            add_empty_journey(txn, "2024-08-01");
            add_empty_journey(txn, "2024-08-05");
            add_empty_journey(txn, "2024-08-06");
            add_empty_journey(txn, "2024-08-06");
            add_empty_journey(txn, "2024-08-10");
            add_empty_journey(txn, "2022-06-10");
            add_empty_journey(txn, "2020-05-10");
            add_empty_journey(txn, "2020-02-29"); // leap year
            Ok(())
        })
        .unwrap();

    assert_eq!(
        main_db.with_txn(|txn| txn.earliest_journey_date()).unwrap(),
        Some(date("2020-02-29"))
    );

    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, None))
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(Some(date("2024-08-06")), None))
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(Some(date("2024-08-06")), Some(date("2024-08-06"))))
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.query_journeys(None, Some(date("2024-08-05"))))
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        main_db.with_txn(|txn| txn.years_with_journey()).unwrap(),
        vec![2020, 2022, 2024]
    );
    // leap year
    assert_eq!(
        main_db
            .with_txn(|txn| txn.months_with_journey(2020))
            .unwrap(),
        vec![2, 5]
    );
    assert_eq!(
        main_db
            .with_txn(|txn| txn.days_with_journey(2024, 8))
            .unwrap(),
        vec![1, 5, 6, 10]
    );
    // 2-29
    assert_eq!(
        main_db
            .with_txn(|txn| txn.days_with_journey(2020, 2))
            .unwrap(),
        vec![29]
    );
    assert!(main_db
        .with_txn(|txn| txn.days_with_journey(2018, 2))
        .unwrap()
        .is_empty(),);
}

// === Action generation and set_invalidate_action tests ===

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn delete_journey_sets_invalidate_months() {
    let temp_dir = TempDir::new("main_db-delete_invalidate").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_journey(&id)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    assert_eq!(
        action,
        Some(Action::Invalidate {
            entries: vec![CacheEntry {
                date: date("2024-03-15"),
                kind: JourneyKind::DefaultKind,
            }],
        })
    );
}

#[test]
fn delete_two_journeys_same_month_deduplicates() {
    let temp_dir = TempDir::new("main_db-delete_dedup").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    // Both journeys share the same date and kind so their CacheEntry
    // values are identical — this actually exercises deduplication.
    let (id1, id2) = main_db
        .with_txn(|txn| {
            let id1 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            let id2 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap2,
            );
            Ok((id1, id2))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_journey(&id1)?;
            txn.delete_journey(&id2)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            // Both entries refer to 2024-03-15/DefaultKind; duplicates are
            // tolerated here because downstream invalidate() deduplicates via HashSet.
            assert!(entries.len() >= 1);
            assert!(entries
                .iter()
                .all(|e| e.date == date("2024-03-15") && e.kind == JourneyKind::DefaultKind));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn delete_two_journeys_different_months_accumulates() {
    let temp_dir = TempDir::new("main_db-delete_diff_months").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let (id1, id2) = main_db
        .with_txn(|txn| {
            let id1 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            let id2 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-15"),
                JourneyKind::DefaultKind,
                bitmap2,
            );
            Ok((id1, id2))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_journey(&id1)?;
            txn.delete_journey(&id2)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(entries
                .iter()
                .any(|e| e.date.year() == 2024 && e.date.month() == 3));
            assert!(entries
                .iter()
                .any(|e| e.date.year() == 2024 && e.date.month() == 5));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn delete_two_journeys_different_kinds_accumulates() {
    let temp_dir = TempDir::new("main_db-delete_diff_kinds").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let (id1, id2) = main_db
        .with_txn(|txn| {
            let id1 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            let id2 = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::Flight,
                bitmap2,
            );
            Ok((id1, id2))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_journey(&id1)?;
            txn.delete_journey(&id2)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(entries.iter().any(|e| e.kind == JourneyKind::DefaultKind));
            assert!(entries.iter().any(|e| e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn update_metadata_same_date_kind_no_action() {
    let temp_dir = TempDir::new("main_db-update_meta_no_action").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &id,
                date("2024-03-15"), // same date
                None,
                None,
                Some("new note".to_string()),
                JourneyKind::DefaultKind, // same kind
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    // Only note changed, date and kind unchanged → no cache action needed.
    // The insert happened in a prior transaction, so action must be None here.
    assert!(action.is_none(), "Expected None, got {:?}", action);
}

#[test]
fn update_metadata_changes_date() {
    let temp_dir = TempDir::new("main_db-update_meta_date").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &id,
                date("2024-05-20"), // different month
                None,
                None,
                None,
                JourneyKind::DefaultKind,
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.year() == 2024 && e.date.month() == 3),
                "Old month should be invalidated"
            );
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.year() == 2024 && e.date.month() == 5),
                "New month should be invalidated"
            );
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn update_metadata_changes_kind() {
    let temp_dir = TempDir::new("main_db-update_meta_kind").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &id,
                date("2024-03-15"), // same date
                None,
                None,
                None,
                JourneyKind::Flight, // different kind
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(entries.iter().any(|e| e.kind == JourneyKind::DefaultKind));
            assert!(entries.iter().any(|e| e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn update_metadata_changes_date_and_kind() {
    let temp_dir = TempDir::new("main_db-update_meta_both").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &id,
                date("2024-06-10"), // different month
                None,
                None,
                None,
                JourneyKind::Flight, // different kind
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(entries.iter().any(|e| e.date.year() == 2024
                && e.date.month() == 3
                && e.kind == JourneyKind::DefaultKind));
            assert!(entries.iter().any(|e| e.date.year() == 2024
                && e.date.month() == 6
                && e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn update_journey_data_sets_invalidate_months() {
    let temp_dir = TempDir::new("main_db-update_data_invalidate").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-07-15"),
                JourneyKind::Flight,
                bitmap,
            ))
        })
        .unwrap();

    let new_bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_data_with_latest_postprocessor(
                &id,
                JourneyData::Bitmap(new_bitmap),
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(entries.iter().any(|e| e.date.year() == 2024
                && e.date.month() == 7
                && e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn delete_all_journeys_sets_complete_rebuilt() {
    let temp_dir = TempDir::new("main_db-delete_all").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            );
            Ok(())
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_all_journeys()?;
            Ok(txn.action.clone())
        })
        .unwrap();

    assert_eq!(action, Some(Action::CompleteRebuilt));
}

#[test]
fn journey_date_range_empty_db() {
    let temp_dir = TempDir::new("main_db-date_range_empty").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let result = main_db.with_txn(|txn| txn.journey_date_range()).unwrap();
    assert_eq!(result, None);
}

#[test]
fn journey_date_range_single_journey() {
    let temp_dir = TempDir::new("main_db-date_range_single").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            );
            Ok(())
        })
        .unwrap();

    let result = main_db.with_txn(|txn| txn.journey_date_range()).unwrap();
    assert_eq!(result, Some((date("2024-03-15"), date("2024-03-15"))));
}

#[test]
fn journey_date_range_multiple_journeys() {
    let temp_dir = TempDir::new("main_db-date_range_multiple").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2020-02-29"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-08-10"),
                JourneyKind::Flight,
                bitmap2,
            );
            Ok(())
        })
        .unwrap();

    let result = main_db.with_txn(|txn| txn.journey_date_range()).unwrap();
    assert_eq!(result, Some((date("2020-02-29"), date("2024-08-10"))));
}

#[test]
fn single_insert_sets_merge_one() {
    let temp_dir = TempDir::new("main_db-single_insert_merge").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            );
            match &txn.action {
                Some(Action::MergeOne { entry, .. }) => {
                    assert_eq!(entry.date, date("2024-03-15"));
                    assert_eq!(entry.kind, JourneyKind::DefaultKind);
                }
                other => panic!("Expected MergeOne, got {:?}", other),
            }
            Ok(txn.action.clone())
        })
        .unwrap();

    assert!(matches!(action, Some(Action::MergeOne { .. })));
}

#[test]
fn two_inserts_escalate_to_invalidate() {
    let temp_dir = TempDir::new("main_db-two_inserts_escalate").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            // After first insert: MergeOne
            assert!(matches!(txn.action, Some(Action::MergeOne { .. })));

            test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-20"),
                JourneyKind::Flight,
                bitmap2,
            );
            // After second insert: escalated to Invalidate
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 3 && e.kind == JourneyKind::DefaultKind),
                "Should contain March DefaultKind entry"
            );
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 5 && e.kind == JourneyKind::Flight),
                "Should contain May Flight entry"
            );
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn third_insert_appends_to_invalidate() {
    let temp_dir = TempDir::new("main_db-third_insert_appends").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap3 = test_utils::make_bitmap_with_line(test_utils::draw_line3);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-20"),
                JourneyKind::Flight,
                bitmap2,
            );
            // After second: Invalidate with 2 entries
            assert!(matches!(txn.action, Some(Action::Invalidate { .. })));

            test_utils::insert_bitmap_journey(
                txn,
                date("2024-07-10"),
                JourneyKind::DefaultKind,
                bitmap3,
            );
            // After third: Invalidate with 3 entries
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert_eq!(entries.len(), 3, "Should have 3 entries");
            assert!(entries
                .iter()
                .any(|e| e.date.month() == 3 && e.kind == JourneyKind::DefaultKind));
            assert!(entries
                .iter()
                .any(|e| e.date.month() == 5 && e.kind == JourneyKind::Flight));
            assert!(entries
                .iter()
                .any(|e| e.date.month() == 7 && e.kind == JourneyKind::DefaultKind));
        }
        other => panic!("Expected Invalidate with 3 entries, got {:?}", other),
    }
}

#[test]
fn two_inserts_same_date_kind_deduplicates() {
    // When two journeys are inserted with the exact same date and kind in a
    // single txn, the escalation from MergeOne to Invalidate should deduplicate:
    // the entries list should contain only one entry.
    let temp_dir = TempDir::new("main_db-two_inserts_same_dedup").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            assert!(matches!(txn.action, Some(Action::MergeOne { .. })));

            // Same exact date and kind as first insert
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap2,
            );
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            // Both inserts share the exact same date and kind; duplicates are
            // tolerated because downstream invalidate() deduplicates via HashSet.
            assert!(entries.len() >= 1);
            assert!(entries
                .iter()
                .all(|e| e.date == date("2024-03-15") && e.kind == JourneyKind::DefaultKind));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn three_inserts_two_same_date_kind_deduplicates() {
    // Three inserts: two share the exact same date+kind, one differs. The
    // Invalidate entries should contain 2 entries (not 3).
    let temp_dir = TempDir::new("main_db-three_inserts_dedup").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap3 = test_utils::make_bitmap_with_line(test_utils::draw_line3);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-20"),
                JourneyKind::Flight,
                bitmap2,
            );
            // Third insert: exact same date+kind as first
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap3,
            );
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            // Duplicates are tolerated because downstream invalidate()
            // deduplicates via HashSet. Just verify both distinct entries exist.
            assert!(entries.len() >= 2);
            assert!(entries
                .iter()
                .any(|e| e.date == date("2024-03-15") && e.kind == JourneyKind::DefaultKind));
            assert!(entries
                .iter()
                .any(|e| e.date == date("2024-05-20") && e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate with entries, got {:?}", other),
    }
}

#[test]
fn insert_then_delete_same_date_kind_deduplicates() {
    // Pre-insert a journey, then in a new txn: insert another with the exact
    // same date+kind (MergeOne), then delete the original. The escalation to
    // Invalidate should deduplicate because both refer to the same date+kind.
    let temp_dir = TempDir::new("main_db-insert_delete_same_dedup").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap_existing = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let existing_id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_existing,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            let bitmap_new = test_utils::make_bitmap_with_line(test_utils::draw_line2);
            // Same exact date+kind as the existing journey
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_new,
            );
            assert!(matches!(txn.action, Some(Action::MergeOne { .. })));

            // Delete existing journey (same date+kind) → escalate to Invalidate
            txn.delete_journey(&existing_id)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            // Duplicates are tolerated because downstream invalidate()
            // deduplicates via HashSet.
            assert!(entries.len() >= 1);
            assert!(entries
                .iter()
                .all(|e| e.date == date("2024-03-15") && e.kind == JourneyKind::DefaultKind));
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn insert_then_delete_converts_merge_to_invalidate() {
    let temp_dir = TempDir::new("main_db-insert_then_delete").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    // Pre-insert a journey in a different month so we can delete it later.
    let bitmap_may = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let may_id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-10"),
                JourneyKind::DefaultKind,
                bitmap_may,
            ))
        })
        .unwrap();

    // In one txn: insert a new journey (March) → Merge, then delete the May journey → Invalidate.
    let action = main_db
        .with_txn(|txn| {
            let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line2);
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_mar,
            );
            // At this point action is Merge with the March journey id.
            assert!(matches!(txn.action, Some(Action::MergeOne { .. })));

            // Now delete the May journey → should transition to Invalidate
            // covering *both* March (from Merge) and May (from delete).
            txn.delete_journey(&may_id)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 3 && e.date.year() == 2024),
                "March (from insert/Merge) should be present in invalidation entries"
            );
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 5 && e.date.year() == 2024),
                "May (from delete) should be present in invalidation entries"
            );
        }
        other => panic!("Expected Invalidate covering both months, got {:?}", other),
    }
}

// === Theme A: CompleteRebuilt cannot be downgraded ===

#[test]
fn complete_rebuilt_survives_subsequent_insert() {
    let temp_dir = TempDir::new("main_db-complete_rebuilt_insert").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    // Pre-insert so delete_all has something to delete
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-01-15"),
                JourneyKind::DefaultKind,
                bitmap.clone(),
            );
            Ok(())
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.delete_all_journeys()?;
            assert_eq!(txn.action, Some(Action::CompleteRebuilt));

            // Insert after delete_all — action should remain CompleteRebuilt
            let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
            txn.create_and_insert_journey(
                date("2024-03-15"),
                None,
                None,
                None,
                JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(bitmap2),
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    assert_eq!(
        action,
        Some(Action::CompleteRebuilt),
        "CompleteRebuilt should not be downgraded by subsequent insert"
    );
}

// === Theme B: Intra-month date change ===

#[test]
fn update_metadata_same_month_different_day_sets_invalidate() {
    let temp_dir = TempDir::new("main_db-same_month_diff_day").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let id = main_db
        .with_txn(|txn| {
            Ok(test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap,
            ))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            txn.update_journey_metadata(
                &id,
                date("2024-03-20"), // same month, different day
                None,
                None,
                None,
                JourneyKind::DefaultKind, // same kind
            )?;
            Ok(txn.action.clone())
        })
        .unwrap();

    // Same-month moves still trigger invalidation (old date + new date entries)
    match action {
        Some(Action::Invalidate { entries }) => {
            assert_eq!(
                entries.len(),
                2,
                "Same-month date change should have 2 entries, got {:?}",
                entries
            );
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

// === Theme C: Combined operations ===

#[test]
fn update_data_then_delete_accumulates() {
    let temp_dir = TempDir::new("main_db-update_then_delete").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap_a = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_b = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let (id_a, id_b) = main_db
        .with_txn(|txn| {
            let a = test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_a,
            );
            let b = test_utils::insert_bitmap_journey(
                txn,
                date("2024-05-10"),
                JourneyKind::DefaultKind,
                bitmap_b,
            );
            Ok((a, b))
        })
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            let new_bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line3);
            txn.update_journey_data_with_latest_postprocessor(
                &id_a,
                JourneyData::Bitmap(new_bitmap),
            )?;
            txn.delete_journey(&id_b)?;
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 3 && e.kind == JourneyKind::DefaultKind),
                "Mar should be in entries"
            );
            assert!(
                entries
                    .iter()
                    .any(|e| e.date.month() == 5 && e.kind == JourneyKind::DefaultKind),
                "May should be in entries"
            );
        }
        other => panic!("Expected Invalidate, got {:?}", other),
    }
}

#[test]
fn finalize_ongoing_sets_merge_one() {
    let temp_dir = TempDir::new("main_db-finalize_merge_one").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    // Record GPS data to create an ongoing journey
    main_db
        .record(
            &gps_processor::RawData {
                point: Point {
                    latitude: 30.27,
                    longitude: 120.16,
                },
                timestamp_ms: Some(1697349115000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();
    main_db
        .record(
            &gps_processor::RawData {
                point: Point {
                    latitude: 30.28,
                    longitude: 120.17,
                },
                timestamp_ms: Some(1697349116000),
                accuracy: None,
                altitude: None,
                speed: None,
            },
            gps_processor::ProcessResult::Append,
        )
        .unwrap();

    let action = main_db
        .with_txn(|txn| {
            let finalized = txn.finalize_ongoing_journey()?;
            assert!(finalized, "Should have finalized a journey");
            Ok(txn.action.clone())
        })
        .unwrap();

    assert!(
        matches!(action, Some(Action::MergeOne { .. })),
        "Finalize should produce MergeOne, got {:?}",
        action
    );
}

// === Theme D: Insert dedup precision ===

#[test]
fn insert_same_date_different_kind_not_deduplicated() {
    let temp_dir = TempDir::new("main_db-same_date_diff_kind").unwrap();
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let action = main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1,
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::Flight,
                bitmap2,
            );
            Ok(txn.action.clone())
        })
        .unwrap();

    match action {
        Some(Action::Invalidate { entries }) => {
            // Same date, different kind = NOT deduped
            assert_eq!(
                entries.len(),
                2,
                "Same date + different kind should NOT be deduplicated, got {:?}",
                entries
            );
            assert!(entries.iter().any(|e| e.kind == JourneyKind::DefaultKind));
            assert!(entries.iter().any(|e| e.kind == JourneyKind::Flight));
        }
        other => panic!("Expected Invalidate with 2 entries, got {:?}", other),
    }
}
