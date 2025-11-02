pub mod test_utils;

use core::panic;

use chrono::Utc;
use memolanes_core::{
    cache_db::{CacheDb, LayerKind},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    main_db::MainDb,
    merged_journey_builder,
};
use tempdir::TempDir;

#[test]
fn basic() {
    let cache_dir = TempDir::new("cache_db-basic").unwrap();
    println!("cache dir: {:?}", cache_dir.path());

    let cache_db = CacheDb::open(cache_dir.path().to_str().unwrap());

    let temp_dir = TempDir::new("main_db-journey_query").unwrap();
    println!("temp dir: {:?}", temp_dir.path());

    let mut main_db = MainDb::open(temp_dir.path().to_str().unwrap());
    let mut journey_bitmap = test_utils::draw_sample_bitmap();
    let journey_kind = JourneyKind::DefaultKind;

    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                journey_kind,
                None,
                JourneyData::Bitmap(journey_bitmap.clone()),
            )
        })
        .unwrap();

    let journey_bitmap_flight = test_utils::draw_sample_bitmap();
    let journey_kind_flight = JourneyKind::Flight;

    main_db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                Utc::now().date_naive(),
                None,
                None,
                None,
                journey_kind_flight,
                None,
                JourneyData::Bitmap(journey_bitmap_flight.clone()),
            )
        })
        .unwrap();

    journey_bitmap.merge(journey_bitmap_flight);
    assert_eq!(
        merged_journey_builder::get_latest(&mut main_db, &cache_db, &Some(LayerKind::All), true)
            .unwrap(),
        journey_bitmap
    );

    assert_eq!(
        cache_db
            .get_full_journey_cache_or_compute(&LayerKind::All, || panic!("should not be called"))
            .unwrap(),
        journey_bitmap
    );
}
