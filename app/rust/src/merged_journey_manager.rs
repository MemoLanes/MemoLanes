/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journeys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/

// TODO: Right now, we just compute everything on demand and this is very slow
// because it needs to go though every existing journey.
// To improve this, we should build some kind of cache:
// V0: we should keep a cache on disk for all finalized journeys. So on startup,
// it just need to load the cache and append things from the current ongoing
// journey which should be small.
// V1: we might need multiple caches for different layer (e.g. one for flight,
// one for land).
// V2: we might need multiple caches keyed by `(start_time, end_time]`, and
// have one per year or one per month. So when there is an update to the
// history, we could clear some but not all cache, and re-construct these
//  outdated ones reasonably quickly.

// current cache design
// read data
// 1. in api.rs, obtain locks for main db and cache db
// 2. try to fetch bitmap from cache db
// 3.a. if no, then fetch bitmap from main db, store it to cache db
// 3.b. if yes, use cached bitmap
// 4. merge bitmap with ongoing journey

// write data
// 1. When a journey is finalized in storage, it will be inserted into main db with txn
// 2. for V0, we simply delete cached bitmap
// TODO: more fine-grained deletes for cache

use crate::{
    cache_db::CacheDb, journey_bitmap::JourneyBitmap, journey_data::JourneyData,
    journey_vector::JourneyVector, main_db::MainDb,
};
use anyhow::Result;
use std::io::{Error, ErrorKind};

fn add_journey_vector_to_journey_bitmap(
    journey_bitmap: &mut JourneyBitmap,
    journey_vector: &JourneyVector,
) {
    for track_segmant in &journey_vector.track_segments {
        for (i, point) in track_segmant.track_points.iter().enumerate() {
            let prev_idx = if i >= 1 { i - 1 } else { 0 };
            let prev = &track_segmant.track_points[prev_idx];
            journey_bitmap.add_line(
                prev.longitude,
                prev.latitude,
                point.longitude,
                point.latitude,
            );
        }
    }
}

pub fn get_latest_including_ongoing(
    main_db: &mut MainDb,
    cache_db: &mut CacheDb,
) -> Result<JourneyBitmap> {
    let ongoing_journey_bitmap = get_ongoing_from_maindb(main_db).unwrap();
    let mut journey_bitmap = match get_finalized_from_cachedb(cache_db) {
        Ok(bitmap) => bitmap,
        Err(e) => {
            if e.to_string() == "Query returned no rows" {
                // If no rows found in cache, fetch from main db and cache the result
                debug!("No journey data found in the cache.");
                let fetched_bitmap = get_finalized_from_maindb(main_db)?;
                insert_cachedb(cache_db, fetched_bitmap.clone());
                fetched_bitmap
            } else {
                debug!("Unexpected error when retrieving cached data: {:?}", e);
                return Err(e);
            }
        }
    };

    journey_bitmap.merge(ongoing_journey_bitmap);
    Ok(journey_bitmap)
}

pub fn insert_cachedb(cache_db: &mut CacheDb, journey_bitmap: JourneyBitmap) {
    // Serialize and write to the cache
    let journey_data = JourneyData::Bitmap(journey_bitmap);
    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();
    cache_db.insert_journey_bitmap(buf).unwrap()
}

pub fn get_ongoing_from_maindb(main_db: &mut MainDb) -> Result<JourneyBitmap> {
    let mut journey_bitmap = JourneyBitmap::new();

    main_db.with_txn(|txn| {
        // ongoing journey
        match txn.get_ongoing_journey()? {
            None => (),
            Some(ongoing_journey) => {
                add_journey_vector_to_journey_bitmap(
                    &mut journey_bitmap,
                    &ongoing_journey.journey_vector,
                );
            }
        }

        Ok(journey_bitmap)
    })
}

pub fn get_finalized_from_maindb(main_db: &mut MainDb) -> Result<JourneyBitmap> {
    let mut journey_bitmap = JourneyBitmap::new();

    main_db.with_txn(|txn| {
        // finalized journeys
        for journey_header in txn.list_all_journeys()? {
            let journey_data = txn.get_journey(&journey_header.id)?;
            match journey_data {
                JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
                JourneyData::Vector(vector) => {
                    add_journey_vector_to_journey_bitmap(&mut journey_bitmap, &vector);
                }
            }
        }

        Ok(journey_bitmap)
    })
}

pub fn get_finalized_from_cachedb(cache_db: &mut CacheDb) -> Result<JourneyBitmap> {
    let mut journey_bitmap = JourneyBitmap::new();
    let journey_data = cache_db.get_journey()?;
    match journey_data {
        JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
        _ => return Err(Error::new(ErrorKind::InvalidData, "Expected bitmap data").into()),
    }

    Ok(journey_bitmap)
}
