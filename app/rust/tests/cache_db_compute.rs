//! What a [`CacheDb`] *answers*, not how it stores it: the bitmap
//! `get_or_compute` returns for a layer over the full range (cached) or an
//! explicit date window (uncached), and that `merge_journey` / `invalidate` /
//! `clear_all` leave subsequent answers correct.
//!
//! Assertions here are on returned values. Which rows the cache actually
//! writes, hits and evicts is the other axis, covered by each backend's
//! own tests.
//!
//! The kind->LayerKind mapping and the renderer's ongoing-journey overlay live
//! in `Storage` and are covered by `tests/storage.rs`.
pub mod test_utils;

use chrono::{NaiveDate, Utc};
use memolanes_core::{
    cache_db::{self, CacheDb, CacheEntry, LayerKind},
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

    let mut cache_db = cache_db::new(cache_dir.path().to_str().unwrap());

    let temp_dir = TempDir::new("main_db-journey_query").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap()).unwrap();
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("full_empty_db");

    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None))
        .unwrap();

    assert_eq!(result, JourneyBitmap::new());
}

#[test]
fn full_with_kind_filter() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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
fn full_repeated_returns_the_same_answer() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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

    // First call computes; second is served from whatever the backend cached.
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
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
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

#[test]
fn merge_journey_noop_on_missing() {
    let cache_dir = TempDir::new("cache_db-merge-noop-missing").unwrap();
    let mut cache_db = cache_db::new(cache_dir.path().to_str().unwrap());

    // No cache exists; merge should be a no-op
    cache_db
        .merge_journey(
            &CacheEntry {
                kind: JourneyKind::DefaultKind,
                date: date("2024-03-15"),
            },
            &JourneyData::Bitmap(JourneyBitmap::new()),
            None,
        )
        .unwrap();
}

#[test]
fn full_cache_aggregates_across_queries() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-full-cache-aggregates");

    let bitmap_jan = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_feb = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line3);
    let bitmap_apr = test_utils::make_bitmap_with_line(test_utils::draw_line4);
    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

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
                date("2024-02-15"),
                JourneyKind::DefaultKind,
                bitmap_feb.clone(),
            );
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

    // Query Jan-Feb
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-01-01"), date("2024-02-29"))),
            )
        })
        .unwrap();

    // Query Mar-Apr
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();

    // Query Jan-Apr: should hit all monthly caches
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-01-01"), date("2024-04-30"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_jan;
    expected.merge(bitmap_feb);
    expected.merge(bitmap_mar);
    expected.merge(bitmap_apr);
    assert_eq!(
        result, expected,
        "Full query must return aggregated data from all months"
    );

    // Repeated query: same result
    let result2 = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-01-01"), date("2024-04-30"))),
            )
        })
        .unwrap();
    assert_eq!(result2, expected);
}

#[test]
fn get_or_compute_explicit_range() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-explicit-range");

    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_apr = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

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

    // Query only Mar
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();
    assert_eq!(result, bitmap_mar);

    // Query Mar+Apr
    let result2 = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();
    let mut expected = bitmap_mar;
    expected.merge(bitmap_apr);
    assert_eq!(result2, expected);
}

#[test]
fn repeated_multi_month_query_uses_monthly_cache() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-repeated-multi-month");

    let bitmap_jan = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_feb = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line3);
    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

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
                date("2024-02-15"),
                JourneyKind::DefaultKind,
                bitmap_feb.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_mar.clone(),
            );
            Ok(())
        })
        .unwrap();

    // First call: compute Jan-Mar
    let result1 = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-01-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_jan;
    expected.merge(bitmap_feb);
    expected.merge(bitmap_mar);
    assert_eq!(result1, expected);

    // Second call: same range — monthly caches serve the data
    let result2 = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-01-01"), date("2024-03-31"))),
            )
        })
        .unwrap();
    assert_eq!(result2, expected);
}

#[test]
fn all_rebuilds_from_per_kind_monthly_caches() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-all-from-monthly");

    let bitmap_default = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_flight = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    let default_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);
    let flight_kind = LayerKind::JourneyKind(JourneyKind::Flight);
    let all_kind = LayerKind::All;

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

    // Populate per-kind monthly caches for Mar 2024
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &default_kind,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )?;
            cache_db.get_or_compute(
                txn,
                &flight_kind,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )?;
            Ok(())
        })
        .unwrap();

    // Query All: should rebuild from per-kind monthly caches
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &all_kind,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_default;
    expected.merge(bitmap_flight);
    assert_eq!(result, expected, "All should merge per-kind monthly caches");
}

#[test]
fn all_rebuilds_from_per_kind_multi_month() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-all-multi-month");

    let bitmap_default_mar = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_default_apr = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let bitmap_flight_mar = test_utils::make_bitmap_with_line(test_utils::draw_line3);
    let bitmap_flight_apr = test_utils::make_bitmap_with_line(test_utils::draw_line4);

    let default_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);
    let flight_kind = LayerKind::JourneyKind(JourneyKind::Flight);
    let all_kind = LayerKind::All;

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap_default_mar.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-04-15"),
                JourneyKind::DefaultKind,
                bitmap_default_apr.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-20"),
                JourneyKind::Flight,
                bitmap_flight_mar.clone(),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-04-20"),
                JourneyKind::Flight,
                bitmap_flight_apr.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Populate per-kind monthly caches for Mar+Apr
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &default_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )?;
            cache_db.get_or_compute(
                txn,
                &flight_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )?;
            Ok(())
        })
        .unwrap();

    // Query All for Mar+Apr: no recomputation needed
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &all_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();

    let mut expected = bitmap_default_mar;
    expected.merge(bitmap_default_apr);
    expected.merge(bitmap_flight_mar);
    expected.merge(bitmap_flight_apr);
    assert_eq!(result, expected);
}

#[test]
fn invalidate_then_requery_returns_correct_data() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-invalidate-requery");

    let bitmap_mar = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_apr = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    let mar_id = main_db
        .with_txn(|txn| {
            let id = test_utils::insert_bitmap_journey(
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
            Ok(id)
        })
        .unwrap();

    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

    // Populate cache
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();

    // Delete Mar journey and invalidate
    main_db
        .with_txn(|txn| {
            txn.delete_journey(&mar_id)?;
            Ok(())
        })
        .unwrap();
    cache_db
        .invalidate(&[CacheEntry {
            date: date("2024-03-15"),
            kind: JourneyKind::DefaultKind,
        }])
        .unwrap();

    // Re-query: should return only Apr bitmap
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &layer_kind,
                Some((date("2024-03-01"), date("2024-04-30"))),
            )
        })
        .unwrap();
    assert_eq!(result, bitmap_apr);
}

#[test]
fn invalidate_one_kind_all_layer_requery_correct() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-invalidate-kind-all");

    let bitmap_default = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap_flight = test_utils::make_bitmap_with_line(test_utils::draw_line2);

    let default_id = main_db
        .with_txn(|txn| {
            let id = test_utils::insert_bitmap_journey(
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
            Ok(id)
        })
        .unwrap();

    // Query All to populate caches
    main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &LayerKind::All,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();

    // Delete DefaultKind journey and invalidate
    main_db
        .with_txn(|txn| {
            txn.delete_journey(&default_id)?;
            Ok(())
        })
        .unwrap();
    cache_db
        .invalidate(&[CacheEntry {
            date: date("2024-03-15"),
            kind: JourneyKind::DefaultKind,
        }])
        .unwrap();

    // Re-query All for Mar: should return only Flight bitmap
    let result = main_db
        .with_txn(|txn| {
            cache_db.get_or_compute(
                txn,
                &LayerKind::All,
                Some((date("2024-03-01"), date("2024-03-31"))),
            )
        })
        .unwrap();
    assert_eq!(result, bitmap_flight);
}

#[test]
fn merge_journey_visible_in_later_query() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-merge-updates");

    let bitmap1 = test_utils::make_bitmap_with_line(test_utils::draw_line1);
    let bitmap2 = test_utils::make_bitmap_with_line(test_utils::draw_line2);
    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2024-03-15"),
                JourneyKind::DefaultKind,
                bitmap1.clone(),
            );
            Ok(())
        })
        .unwrap();

    // Populate cache
    main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None))
        .unwrap();

    // Merge a new bitmap
    cache_db
        .merge_journey(
            &CacheEntry {
                kind: JourneyKind::DefaultKind,
                date: date("2024-03-20"),
            },
            &JourneyData::Bitmap(bitmap2.clone()),
            None,
        )
        .unwrap();

    // Cache should now contain merged result
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None))
        .unwrap();
    let mut expected = bitmap1;
    expected.merge(bitmap2);
    assert_eq!(result, expected);
}

#[test]
fn clear_all_then_requery_recomputes() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-clear-all");

    let bitmap = test_utils::draw_sample_bitmap();
    let layer_kind = LayerKind::JourneyKind(JourneyKind::DefaultKind);

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

    // Populate cache
    main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None))
        .unwrap();

    // Clear
    cache_db.clear_all().unwrap();

    // Re-query: should recompute
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None))
        .unwrap();
    assert_eq!(result, bitmap);
}
