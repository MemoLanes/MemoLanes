pub mod test_utils;

use memolanes_core::{
    cache_db::{CacheDb, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_header::JourneyKind,
};
use tempdir::TempDir;

// TODO: The whole caching is critical and error prone, we should have better test coverage.

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;

    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JourneyKind(journey_kind), || {
                Ok(journey_bitmap.clone())
            })
            .unwrap(),
        journey_bitmap
    );
    // it should be saved in the cache
    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JourneyKind(journey_kind), || panic!(
                "Should not be called"
            ))
            .unwrap(),
        journey_bitmap
    );
    // test clear cache

    cache_db.clear_all_cache().unwrap();
    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JourneyKind(journey_kind), || {
                Ok(JourneyBitmap::new())
            })
            .unwrap(),
        JourneyBitmap::new()
    );

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    // no-op since there is no cache
    cache_db
        .update_full_journey_cache_if_exists(&LayerKind::JourneyKind(journey_kind_flight), |_| {
            panic!("Should not be called")
        })
        .unwrap();

    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JourneyKind(journey_kind_flight), || Ok(
                journey_bitmap_flight.clone()
            ))
            .unwrap(),
        journey_bitmap_flight
    );
}

#[test]
fn reopen_cache() {
    let cache_dir = TempDir::new("cache_db_reopen").unwrap();
    let cache_path = cache_dir.path().join("cache.db");

    let db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let bitmap = JourneyBitmap::new();
    db.set_full_journey_cache(&LayerKind::JourneyKind(JourneyKind::DefaultKind), &bitmap)
        .unwrap();

    std::fs::remove_file(&cache_path).unwrap();
    assert!(!cache_path.exists(), "DB file should be deleted");

    let bitmap2 = JourneyBitmap::new();
    db.set_full_journey_cache(&LayerKind::JourneyKind(JourneyKind::Flight), &bitmap2)
        .unwrap();
    assert!(
        cache_path.exists(),
        "DB file should be recreated after write"
    );

    std::fs::write(&cache_path, b"corrupted content").unwrap();

    let bitmap3 = JourneyBitmap::new();
    db.set_full_journey_cache(&LayerKind::All, &bitmap3)
        .unwrap();

    let read_bitmap = db.get_full_journey_cache(&LayerKind::All).unwrap().unwrap();
    assert_eq!(read_bitmap, bitmap3);
}
