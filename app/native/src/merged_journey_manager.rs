/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/

// TODO: Right now, we just compute everything on demand and this is very slow
// because it needs to go though every existing journey.
// To improve this, we should build some kind of cache:
// V0: we should keep a cache on disk for all finalized journeys. So on startup,
// it just need to load the cache and append things from the current ongoing
// journy which should be small.
// V1: we might need multiple caches for different layer (e.g. one for flight,
// one for land).
// V2: we might need multiple caches keyed by `(start_time, end_time]`, and
// have one per year or one per month. So when there is an update to the
// history, we could clear some but not all cache, and re-construct these
//  outdated ones reasonably quickly.

use crate::{
    journey_bitmap::JourneyBitmap, journey_data::JourneyData, journey_vector::JourneyVector,
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

pub fn get_latest_including_ongoing(main_db: &mut MainDb) -> Result<JourneyBitmap> {
    let mut journey_bitmap = JourneyBitmap::new();

    // finalized journeys
    for journey_header in main_db.list_all_journeys()? {
        let journey_data = main_db.get_journey(&journey_header.id)?;
        match journey_data {
            JourneyData::Bitmap(_bitmap) => {
                panic!("unimplemented");
            }
            JourneyData::Vector(vector) => {
                add_journey_vector_to_journey_bitmap(&mut journey_bitmap, &vector);
            }
        }
    }

    // ongoing journey
    match main_db.get_ongoing_journey()? {
        None => (),
        Some((_, _, journey_vector)) => {
            add_journey_vector_to_journey_bitmap(&mut journey_bitmap, &journey_vector);
        }
    }

    Ok(journey_bitmap)
}
