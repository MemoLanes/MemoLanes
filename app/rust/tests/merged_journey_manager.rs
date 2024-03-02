pub mod test_utils;
use chrono::Utc;
use memolanes_core::{
    cache_db::CacheDb, journey_bitmap::JourneyBitmap, journey_data::JourneyData, main_db::MainDb,
    merged_journey_manager,
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
                None,
                Utc::now(),
                None,
                memolanes_core::journey_header::JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(journey_bitmap.clone()),
            )
        })
        .unwrap();

    let journey_cache = cache_db.get_journey();
    match journey_cache {
        Err(e) => {
            if e.to_string() == "Query returned no rows" {
                println!("No journey data found in the cache.");
            } else {
                panic!("Unexpected error when retrieving cached data: {:?}", e);
            }
        }
        _ => {
            panic!("Cache Db should start empty")
        }
    };

    // Result from main db, but also fill cache db
    let stored_bitmap =
        merged_journey_manager::get_latest_including_ongoing(&mut main_db, &mut cache_db).unwrap();

    assert_eq!(journey_bitmap, stored_bitmap);

    // validate the cached journey
    let cached_data = cache_db.get_journey().unwrap();

    let mut cached_bitmap = JourneyBitmap::new();
    match cached_data {
        JourneyData::Bitmap(bitmap) => cached_bitmap.merge(bitmap),
        _ => panic!("Expected bitmap data"),
    }

    assert_eq!(journey_bitmap, cached_bitmap);
}
