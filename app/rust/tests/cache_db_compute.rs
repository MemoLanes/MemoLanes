// Computation behaviour of `CacheDb::get_or_compute`: merging stored
// journeys into a bitmap for a layer, over the full range (cached) or an
// explicit date window (uncached). The kind->LayerKind mapping and the
// renderer's ongoing-journey overlay live in `Storage` and are covered
// by `tests/storage.rs`.
pub mod test_utils;

use chrono::{NaiveDate, Utc};
use memolanes_core::{
    cache_db::{CacheDb, CacheDbV1, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    main_db::MainDb,
};
use tempdir::TempDir;

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

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
            )
        })
        .unwrap();

    journey_bitmap.merge(journey_bitmap_flight);
    assert_eq!(
        main_db
            .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
            .unwrap(),
        journey_bitmap
    );

    // Call again — result should be the same (served from cache).
    assert_eq!(
        main_db
            .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
            .unwrap(),
        journey_bitmap
    );
}

// === explicit-range queries ===

#[test]
fn range_full_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_full_month");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn range_partial_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_partial_month");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-10"), date("2024-03-20"))),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn range_cross_month() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_cross_month");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_mar.clone();
    expected.merge(bitmap_apr.clone());
    assert_eq!(result, expected);
}

#[test]
fn range_partial_start_full_middle_partial_end() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_partial_full_partial");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-01-15"), date("2024-03-15"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_jan.clone();
    expected.merge(bitmap_feb.clone());
    expected.merge(bitmap_mar.clone());
    assert_eq!(result, expected);
}

#[test]
fn range_empty_db() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_empty_db");

    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &LayerKind::All,
                Some((date("2024-01-01"), date("2024-12-31"))),
            )
        })
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn range_with_kind_filter() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_kind_filter");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap_default);
    assert_ne!(result, bitmap_flight);
}

#[test]
fn range_cross_year() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_cross_year");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2023-12-01"), date("2024-01-31"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_dec.clone();
    expected.merge(bitmap_jan.clone());
    assert_eq!(result, expected);
}

#[test]
fn range_leap_year_february() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_leap_feb");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-02-01"), date("2024-02-29"))),
            )
        })
        .unwrap();

    assert_eq!(result, bitmap);
}

#[test]
fn range_all_kinds() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_all_kinds");

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

    // LayerKind::All over the range → union of both kinds
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &LayerKind::All,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_default.clone();
    expected.merge(bitmap_flight.clone());
    assert_eq!(result, expected);
}

// === full-range queries (cached) ===

#[test]
fn full_empty_db() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("full_empty_db");

    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn full_with_kind_filter() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("full_kind_filter");

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

    let result_default = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(txn, &LayerKind::JourneyKind(JourneyKind::DefaultKind), None)
        })
        .unwrap();
    assert_eq!(result_default, bitmap_default);

    let result_flight = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(txn, &LayerKind::JourneyKind(JourneyKind::Flight), None)
        })
        .unwrap();
    assert_eq!(result_flight, bitmap_flight);
}

#[test]
fn full_repeated_uses_full_table_cache() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("full_repeated_cache");

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

    // First call computes; second is a cache hit — both equal.
    let result1 = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
        .unwrap();
    let result2 = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
        .unwrap();

    assert_eq!(result1, result2);

    let mut expected = bitmap_jan;
    expected.merge(bitmap_mar);
    assert_eq!(result1, expected);
}

#[test]
fn range_after_insert_and_invalidation() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("range_insert_invalidate");

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
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-01"), date("2024-03-31"))),
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

    cache_db
        .invalidate(&[memolanes_core::cache_db::CacheEntry {
            date: date("2024-03-20"),
            kind: JourneyKind::DefaultKind,
        }])
        .unwrap();

    // Re-query: should return A + B merged (recomputed from MainDb)
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &LayerKind::JourneyKind(JourneyKind::DefaultKind),
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_a;
    expected.merge(bitmap_b);
    assert_eq!(result, expected);
}
