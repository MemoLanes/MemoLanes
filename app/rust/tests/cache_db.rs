pub mod test_utils;

use memolanes_core::{
    cache_db::{CacheDb, JourneyCacheKey}, journey_bitmap::JourneyBitmap, journey_data::JourneyData, journey_header::JourneyKind
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let key = JourneyCacheKey::All;
    let journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;
    
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&key, Some(&journey_kind), || {
                Ok(journey_bitmap.clone())
            })
            .unwrap(),
            journey_bitmap
    );
    // it should be saved in the cache
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&key, Some(&journey_kind), || panic!("Should not be called"))
            .unwrap(),
        journey_bitmap
    );
    // test clear cache

    cache_db.clear_journey_cache().unwrap();
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&key, Some(&journey_kind), || {
                Ok(JourneyBitmap::new())
            })
            .unwrap(),
        JourneyBitmap::new()
    );

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    // upsert
    let _ = cache_db.upsert_journey_cache(&key, &journey_kind_flight, JourneyData::Bitmap(journey_bitmap_flight.clone()));

    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&key, Some(&journey_kind_flight), || panic!("Should not be called"))
            .unwrap(),
            journey_bitmap
    );
}
