/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journeys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/


use std::collections::HashMap;

use crate::{
    cache_db::{CacheDb, JourneyCacheKey}, journey_bitmap::JourneyBitmap, journey_data::JourneyData, journey_header::JourneyKind, journey_vector::JourneyVector, main_db::{self, MainDb}
};
use anyhow::Result;
use chrono::NaiveDate;

pub fn add_journey_vector_to_journey_bitmap(
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


// TODO: This is going to be very slow.
// Returns a collection of journey kind and journey bitmap, leaves freedom to the caller
fn get_range_internal(
    txn: &mut main_db::Txn,
    from_date_inclusive: Option<NaiveDate>,
    to_date_inclusive: Option<NaiveDate>,
) -> Result<HashMap<JourneyKind, JourneyBitmap>> {
    let mut journey_hashmap = HashMap::new();

    for journey_header in txn.query_journeys(from_date_inclusive, to_date_inclusive)? {
        let journey_kind = journey_header.journey_kind;

        let journey_bitmap = journey_hashmap
            .entry(journey_kind)
            .or_insert_with(JourneyBitmap::new);

        let journey_data = txn.get_journey_data(&journey_header.id)?;
        match journey_data {
            JourneyData::Bitmap(bitmap) => journey_bitmap.merge(bitmap),
            JourneyData::Vector(vector) => {
                add_journey_vector_to_journey_bitmap(journey_bitmap, &vector);
            }
        }
    }
    Ok(journey_hashmap)
}


pub fn get_range(
    txn: &mut main_db::Txn,
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
) -> Result<JourneyBitmap> {
    let journey_hashmap = get_range_internal(
        txn,
        Some(from_date_inclusive),
        Some(to_date_inclusive),
    )?;
    let mut journey_bitmap = JourneyBitmap::new();
    for (_, bitmap) in journey_hashmap {
        journey_bitmap.merge(bitmap);
    }
    Ok(journey_bitmap)
}

pub fn get_latest_including_ongoing(
    main_db: &mut MainDb,
    cache_db: &CacheDb,
) -> Result<JourneyBitmap> {
    main_db.with_txn(|txn| {
        // getting finalized journeys
        let mut journey_bitmap = cache_db
            .get_journey_cache_or_compute(&JourneyCacheKey::All, || {
                get_range_internal(txn, None, None)
            })?;

        // append remaining ongoing parts
        match txn.get_ongoing_journey()? {
            None => (),
            Some(ongoing_journey) => add_journey_vector_to_journey_bitmap(
                &mut journey_bitmap,
                &ongoing_journey.journey_vector,
            ),
        }

        // NOTE: Calling to `main_db.with_txn` directly without going through
        // `storage` is fine here because we are not modifying main db here.
        // But just to make sure:
        assert_eq!(txn.action, None);
        Ok(journey_bitmap)
    })
}
