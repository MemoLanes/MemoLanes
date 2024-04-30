/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journeys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/

use crate::{
    cache_db::{CacheDb, JourneyCacheKey},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_vector::JourneyVector,
    main_db::MainDb,
};
use anyhow::Result;

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
    cache_db: &CacheDb,
) -> Result<JourneyBitmap> {
    main_db.with_txn(|txn| {
        // getting finalized journeys
        let mut journey_bitmap =
            cache_db.get_journey_cache_or_compute(&JourneyCacheKey::All, || {
                let mut journey_bitmap = JourneyBitmap::new();
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
        assert!(!txn.reset_cache);
        Ok(journey_bitmap)
    })
}
