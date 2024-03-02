pub mod test_utils;

use memolanes_core::{cache_db::CacheDb, journey_bitmap::JourneyBitmap, journey_data::JourneyData};
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

    assert_eq!(journey_data, journey_cache);

    let mut journey_bitmap_from_cache = JourneyBitmap::new();
    match journey_cache {
        JourneyData::Bitmap(bitmap) => journey_bitmap_from_cache.merge(bitmap),
        _ => panic!("Expected bitmap data"),
    }

    let journey_bitmap = draw_sample_bitmap().unwrap();
    assert_eq!(journey_bitmap, journey_bitmap_from_cache);

    // test delete
    cache_db.delete_cached_journey().unwrap();
    let res_cache_db = cache_db.get_journey();
    match res_cache_db {
        Err(e) => {
            if e.to_string() != "Query returned no rows" {
                panic!("Unexpected error when retrieving cached data: {:?}", e);
            }
        }
        _ => {
            panic!("Cache shold be deleted")
        }
    };

    let mut buf1 = Vec::new();
    journey_data.serialize(&mut buf1).unwrap();
    cache_db.insert_journey_bitmap(buf1).unwrap();
    journey_cache = cache_db.get_journey().unwrap();
    assert_eq!(journey_data, journey_cache);
}
