pub mod test_utils;

use memolanes_core::{
    cache_db::{CacheDb, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_header::JourneyKind,
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;

    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JounreyKind(journey_kind), || {
                Ok(journey_bitmap.clone())
            })
            .unwrap(),
        journey_bitmap
    );
    // it should be saved in the cache
    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JounreyKind(journey_kind), || panic!(
                "Should not be called"
            ))
            .unwrap(),
        journey_bitmap
    );
    // test clear cache

    cache_db.clear_all_cache().unwrap();
    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JounreyKind(journey_kind), || {
                Ok(JourneyBitmap::new())
            })
            .unwrap(),
        JourneyBitmap::new()
    );

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    // no-op since there is no cache
    cache_db
        .update_full_journey_cache_if_exists(&LayerKind::JounreyKind(journey_kind_flight), |_| {
            panic!("Should not be called")
        })
        .unwrap();

    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::JounreyKind(journey_kind_flight), || Ok(
                journey_bitmap_flight.clone()
            ))
            .unwrap(),
        journey_bitmap_flight
    );
}
