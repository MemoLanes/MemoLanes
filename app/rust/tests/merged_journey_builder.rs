pub mod test_utils;

use chrono::{NaiveDate, Utc};
use memolanes_core::{
    cache_db::{CacheDbV1, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    main_db::MainDb,
    merged_journey_builder,
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDbV1::open(cache_dir.path().to_str().unwrap());

    let temp_dir = TempDir::new("main_db-journey_query").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    let mut journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;

    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                journey_kind,
                None,
                JourneyData::Bitmap(journey_bitmap.clone()),
                None,
            )
        })
        .unwrap();

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                journey_kind_flight,
                None,
                JourneyData::Bitmap(journey_bitmap_flight.clone()),
                None,
            )
        })
        .unwrap();

    journey_bitmap.merge(journey_bitmap_flight);
    assert_eq!(
        main_db
            .with_txn(|txn| {
                merged_journey_builder::get_full(txn, &cache_db, &Some(LayerKind::All), true)
            })
            .unwrap(),
        journey_bitmap
    );

    // Call get_full again — result should be the same
    assert_eq!(
        main_db
            .with_txn(|txn| {
                merged_journey_builder::get_full(txn, &cache_db, &Some(LayerKind::All), true)
            })
            .unwrap(),
        journey_bitmap
    );
}

// === get_range tests ===

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn get_range_full_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_full_month");

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap.clone(),
            );
            Ok(())
        })
        .unwrap();

    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn get_range_partial_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_partial_month");

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Partial month query
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-10"),
                date("2024-03-20"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn get_range_cross_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_cross_month");

    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_apr = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_mar.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-04-15"),
                JourneyKind::DefaultKind,
                bitmap_apr.clone(),
            );
            Ok(())
        })
        .unwrap();

    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-04-30"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    let mut expected = bitmap_mar.clone();
    expected.merge(bitmap_apr.clone());
    assert_eq!(result, expected);
}

#[test]
fn get_range_partial_start_full_middle_partial_end() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_partial_full_partial");

    let bitmap_jan = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_feb = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line3);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-01-20"),
                JourneyKind::DefaultKind,
                bitmap_jan.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-02-15"),
                JourneyKind::DefaultKind,
                bitmap_feb.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-10"),
                JourneyKind::DefaultKind,
                bitmap_mar.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Jan 15..Mar 15: partial Jan, full Feb, partial Mar
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-01-15"),
                date("2024-03-15"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    let mut expected = bitmap_jan.clone();
    expected.merge(bitmap_feb.clone());
    expected.merge(bitmap_mar.clone());
    assert_eq!(result, expected);
}

#[test]
fn get_range_empty_db() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_empty_db");

    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-01-01"),
                date("2024-12-31"),
                None,
            )
        })
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn get_range_with_kind_filter() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_kind_filter");

    let bitmap_default = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_flight = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_default.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::Flight,
                bitmap_flight.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Filter by DefaultKind only
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap_default);
    assert_ne!(result, bitmap_flight);
}

#[test]
fn get_range_cross_year() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_cross_year");

    let bitmap_dec = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_jan = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2023-12-15"),
                JourneyKind::DefaultKind,
                bitmap_dec.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-01-15"),
                JourneyKind::DefaultKind,
                bitmap_jan.clone(),
            );
            Ok(())
        })
        .unwrap();

    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2023-12-01"),
                date("2024-01-31"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    let mut expected = bitmap_dec.clone();
    expected.merge(bitmap_jan.clone());
    assert_eq!(result, expected);
}

#[test]
fn get_range_leap_year_february() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_leap_feb");

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-02-29"),
                JourneyKind::DefaultKind,
                bitmap.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Query full Feb 2024 (leap year: Feb 1 - Feb 29)
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-02-01"),
                date("2024-02-29"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn get_full_empty_db() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_full_empty_db");

    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_full(txn, &cache_db, &Some(LayerKind::All), false)
        })
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn get_full_with_kind_filter() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_full_kind_filter");

    let bitmap_default = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_flight = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_default.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::Flight,
                bitmap_flight.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Filter by DefaultKind
    let result_default = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_full(
                txn,
                &cache_db,
                &Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
                false,
            )
        })
        .unwrap();
    assert_eq!(result_default, bitmap_default);

    // Filter by Flight
    let result_flight = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_full(
                txn,
                &cache_db,
                &Some(LayerKind::JourneyKind(JourneyKind::Flight)),
                false,
            )
        })
        .unwrap();
    assert_eq!(result_flight, bitmap_flight);
}

#[test]
fn get_range_all_kinds() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_all_kinds");

    let bitmap_default = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_flight = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_default.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::Flight,
                bitmap_flight.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Query with kind: None → should return union of both kinds
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-03-31"),
                None,
            )
        })
        .unwrap();

    let mut expected = bitmap_default.clone();
    expected.merge(bitmap_flight.clone());
    assert_eq!(result, expected);
}

#[test]
fn get_full_repeated_uses_full_table_cache() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_full_repeated_cache");

    let bitmap_jan = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-01-15"),
                JourneyKind::DefaultKind,
                bitmap_jan.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::Flight,
                bitmap_mar.clone(),
            );
            Ok(())
        })
        .unwrap();

    // First call
    let result1 = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_full(txn, &cache_db, &Some(LayerKind::All), false)
        })
        .unwrap();

    // Second call (cache hit)
    let result2 = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_full(txn, &cache_db, &Some(LayerKind::All), false)
        })
        .unwrap();

    assert_eq!(result1, result2);

    let mut expected = bitmap_jan;
    expected.merge(bitmap_mar);
    assert_eq!(result1, expected);
}

#[test]
fn get_full_with_none_layer_kind_returns_empty() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_full_none_layer");

    let bitmap = test_utils::make_bitmap_with_line(test_utils::draw_line1);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap.clone(),
            );
            Ok(())
        })
        .unwrap();

    // layer_kind = None → disabled layer, returns empty bitmap
    let result = main_db
        .with_txn(|txn| merged_journey_builder::get_full(txn, &cache_db, &None, false))
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn get_range_after_insert_and_invalidation() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("get_range_insert_invalidate");

    let bitmap_a = test_utils::make_bitmap_with_line(test_utils::draw_line1);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-10"),
                JourneyKind::DefaultKind,
                bitmap_a.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Populate cache
    main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    // Insert B in Mar and invalidate
    let bitmap_b = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::DefaultKind,
                bitmap_b.clone(),
            );
            Ok(())
        })
        .unwrap();

    use memolanes_core::cache_db::{CacheDb, CacheEntry};
    cache_db
        .invalidate(&[CacheEntry {
            date: date("2024-03-20"),
            kind: JourneyKind::DefaultKind,
        }])
        .unwrap();

    // Re-query: should return A + B merged (recomputed from MainDb)
    let result = main_db
        .with_txn(|txn| {
            merged_journey_builder::get_range(
                txn,
                &cache_db,
                date("2024-03-01"),
                date("2024-03-31"),
                Some(&JourneyKind::DefaultKind),
            )
        })
        .unwrap();

    let mut expected = bitmap_a;
    expected.merge(bitmap_b);
    assert_eq!(result, expected);
}
