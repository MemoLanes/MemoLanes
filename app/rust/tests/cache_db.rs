pub mod test_utils;

use memolanes_core::{
    cache_db::{CacheDb, JourneyCacheKey},
    journey_bitmap::JourneyBitmap,
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let journey_bitmap = test_utils::draw_sample_bitmap();
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || Ok(journey_bitmap.clone()))
            .unwrap(),
        journey_bitmap
    );
    // it should be saved in the cache
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || panic!("Should not be called"))
            .unwrap(),
        journey_bitmap
    );
    // test clear cache
    cache_db.clear_journey_cache().unwrap();
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || Ok(JourneyBitmap::new()))
            .unwrap(),
        JourneyBitmap::new()
    );
}
