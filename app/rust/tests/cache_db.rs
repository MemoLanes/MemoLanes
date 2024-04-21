pub mod test_utils;

use memolanes_core::{cache_db::CacheDb, journey_data::JourneyData};
use tempdir::TempDir;

use crate::test_utils::draw_sample_bitmap;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let journey_bitmap = draw_sample_bitmap().unwrap();
    let journey_data = JourneyData::Bitmap(journey_bitmap);

    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();

    println!("size: {}", buf.len());

    cache_db.insert_journey_bitmap(buf).unwrap();

    // validate the cached journey
    let mut journey_cache = cache_db.get_journey().unwrap();

    let journey_bitmap = draw_sample_bitmap().unwrap();
    match journey_cache {
        Some(journey_cache) => assert_eq!(journey_bitmap, journey_cache),
        _ => panic!("Expected bitmap data"),
    }

    // test delete
    cache_db.delete_cached_journey().unwrap();
    let result = cache_db.get_journey();
    match result {
        Ok(None) => (),
        Ok(Some(_)) => panic!("Expected no bitmap but found one."),
        Err(e) => panic!("Expected no bitmap but encountered an error: {}", e),
    }

    let mut buf1 = Vec::new();
    journey_data.serialize(&mut buf1).unwrap();
    cache_db.insert_journey_bitmap(buf1).unwrap();
    journey_cache = cache_db.get_journey().unwrap();
    let journey_bitmap = draw_sample_bitmap().unwrap();
    match journey_cache {
        Some(journey_cache) => assert_eq!(journey_bitmap, journey_cache),
        _ => panic!("Expected bitmap data"),
    }
}
