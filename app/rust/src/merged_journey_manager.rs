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
// 2. check cache presence and validity by comparing timestamps between app's and cache db's
// (this cache timestamp can also be cached in the manager)
// 3. a. if no, then fetch from main db, store it to cache db and return bitmap
// 3. a. 1, currently main db provides a de-serialized result, which has to be serialized again for cache db
// TODO: so a better way is main db returns a blob, manager insert it directly to cache db, and then de-serialized for app
// 3.b. if yes, then fetch from cache db and return

// write data
// 1. app keeps the latest timestamp when it updates bitmap related data in main db
// (This means finalize any ongoing journey?)
// 2. This new timestamp implicitly out-dates cache's timestamp
// 3. Question: when to GC cache db? after write? after read? async?

// TODO: add tests for cache db

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

pub fn get_latest_including_ongoing_from_cache(cache_db: &mut CacheDb) -> Result<JourneyBitmap> {
    let mut journey_bitmap = JourneyBitmap::new();
    cache_db.with_txn(|txn| {
        let journey_data = txn.get_journey()?;
        match journey_data {
            JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
            _ => return Err(Error::new(ErrorKind::InvalidData, "Expected bitmap data").into()),
        }

        Ok(journey_bitmap)
    })
}

pub fn get_latest_including_ongoing(
    main_db: &mut MainDb,
    cache_db: &mut CacheDb,
) -> Result<JourneyBitmap> {
    if let Ok(journey_bitmap) = get_latest_including_ongoing_from_cache(cache_db) {
        if !journey_bitmap.tiles.is_empty() {
            // If found and not empty, return it immediately
            return Ok(journey_bitmap);
        }
    }

    let journey_bitmap = get_latest_including_ongoing_from_maindb(main_db).unwrap();

    // Serialize and write to the cache
    let journey_data = JourneyData::Bitmap(journey_bitmap);
    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();

    // TODO: use last updated timestamp
    //let current_timestamp = Utc::now();

    cache_db.with_txn(|txn| {
        // serialized data into the cache database
        txn.insert_journey_bitmap_blob(buf)?;
        Ok(())
    })?;

    match journey_data {
        JourneyData::Bitmap(journey_bitmap) => Ok(journey_bitmap),
        _ => Err(Error::new(ErrorKind::InvalidData, "Expected bitmap data").into()),
    }
}

pub fn get_latest_including_ongoing_from_maindb(main_db: &mut MainDb) -> Result<JourneyBitmap> {
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
