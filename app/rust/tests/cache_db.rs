pub mod test_utils;

use chrono::NaiveDate;
use memolanes_core::{
    cache_db::{CacheDb, CacheDbV1, CacheEntry, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
};
use tempdir::TempDir;

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn merge_journey_noop_on_missing() {
    let cache_dir = TempDir::new("cache_db-merge-noop-missing").unwrap();
    let cache_db = CacheDbV1::open(cache_dir.path().to_str().unwrap());

    // No cache exists; merge should be a no-op
    cache_db
        .merge_journey(
            &CacheEntry {
                kind: JourneyKind::DefaultKind,
                date: date("2024-03-15"),
            },
            &JourneyData::Bitmap(JourneyBitmap::new()),
        )
        .unwrap();
}

#[test]
fn get_or_compute_full_range() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-full-range");

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

    // First call: cache miss
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();
    assert_eq!(result, bitmap);

    // Second call: cache hit
    let result2 = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();
    assert_eq!(result2, bitmap);
}

#[test]
fn get_or_compute_explicit_range() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
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
                Some(date("2024-03-01")),
                Some(date("2024-03-31")),
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
                Some(date("2024-03-01")),
                Some(date("2024-04-30")),
            )
        })
        .unwrap();
    let mut expected = bitmap_mar;
    expected.merge(bitmap_apr);
    assert_eq!(result2, expected);
}

#[test]
fn all_layer_merges_per_kind() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
        test_utils::setup_main_and_cache_db("cache_db-all-merges-kinds");

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

    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None, None))
        .unwrap();

    let mut expected = bitmap_default;
    expected.merge(bitmap_flight);
    assert_eq!(result, expected);
}

#[test]
fn invalidate_then_requery_returns_correct_data() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
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
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
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
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();
    assert_eq!(result, bitmap_apr);
}

#[test]
fn invalidate_one_kind_all_layer_requery_correct() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
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
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None, None))
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

    // Re-query All: should return only Flight bitmap
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &LayerKind::All, None, None))
        .unwrap();
    assert_eq!(result, bitmap_flight);
}

#[test]
fn merge_journey_updates_existing_cache() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
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
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();

    // Merge a new bitmap
    cache_db
        .merge_journey(
            &CacheEntry {
                kind: JourneyKind::DefaultKind,
                date: date("2024-03-20"),
            },
            &JourneyData::Bitmap(bitmap2.clone()),
        )
        .unwrap();

    // Cache should now contain merged result
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();
    let mut expected = bitmap1;
    expected.merge(bitmap2);
    assert_eq!(result, expected);
}

#[test]
fn clear_all_removes_cache() {
    let (mut main_db, cache_db, _main_dir, _cache_dir) =
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
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();

    // Clear
    cache_db.clear_all().unwrap();

    // Re-query: should recompute
    let result = main_db
        .with_txn(|txn| cache_db.get_or_compute(txn, &layer_kind, None, None))
        .unwrap();
    assert_eq!(result, bitmap);
}
