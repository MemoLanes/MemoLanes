pub mod test_utils;

use std::collections::HashMap;

use memolanes_core::{
    cache_db::{self, CacheDb, JourneyCacheKey},
    journey_bitmap::JourneyBitmap, journey_header::{JourneyHeader, JourneyKind},
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;
    let mut expected_map = HashMap::new();
    expected_map.insert(journey_kind.clone(), journey_bitmap.clone());
    
    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
                Ok(expected_map.clone())
            })
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
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
                Ok(HashMap::new())
            })
            .unwrap(),
        JourneyBitmap::new()
    );

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    let journey_header = JourneyHeader::new();


    let cache_db.merge_journey_cache(&JourneyCacheKey::All, ||)

    let _ = cache_db
    .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
        Ok((journey_kind.clone(), journey_bitmap.clone()))
    });

    let _ = cache_db
    .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
        Ok((journey_kind_flight.clone(), journey_bitmap_flight.clone()))
    });


    assert_eq!(
        cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
                Ok((journey_kind_flight.clone(), journey_bitmap.clone()))
            })
            .unwrap(),
        journey_bitmap
    );
}
