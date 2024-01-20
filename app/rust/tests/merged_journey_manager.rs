pub mod test_utils;
use chrono::Utc;
use rust_lib::{
    cache_db::CacheDb, journey_bitmap::JourneyBitmap, journey_data::JourneyData,
    journey_header::JourneyKind, main_db::MainDb, merged_journey_manager,
};
use tempdir::TempDir;

use crate::test_utils::draw_sample_bitmap;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());
    let mut cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let temp_dir = TempDir::new("main_db-basic").unwrap();
    println!("temp dir: {:?}", temp_dir.path());
    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());

    let journey_bitmap = draw_sample_bitmap().unwrap();

    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Some(Utc::now()),
                Utc::now(),
                Some(Utc::now()),
                JourneyKind::Default,
                Some("Sample note".to_string()),
                JourneyData::Bitmap(journey_bitmap),
            )
        })
        .unwrap();

    let journey_bitmap_from_db =
        merged_journey_manager::get_latest_including_ongoing(&mut main_db, &mut cache_db).unwrap();
    let journey_bitmap = draw_sample_bitmap().unwrap();

    assert_eq!(journey_bitmap, journey_bitmap_from_db);

    // validate the cached journey
    let journey_cache = cache_db.with_txn(|txn| txn.get_journey()).unwrap();

    let mut journey_bitmap_from_cache = JourneyBitmap::new();
    match journey_cache {
        JourneyData::Bitmap(bitmap) => journey_bitmap_from_cache.merge(bitmap),
        _ => panic!("Expected bitmap data"),
    }

    assert_eq!(journey_bitmap, journey_bitmap_from_cache);

    let journey_bitmap_from_db_2nd =
        merged_journey_manager::get_latest_including_ongoing(&mut main_db, &mut cache_db).unwrap();
    assert_eq!(journey_bitmap_from_db_2nd, journey_bitmap_from_cache);
}

// TODO: add tests for modifying results and invalidate cache
