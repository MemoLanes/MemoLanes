/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journeys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/
use crate::{
    cache_db::{CacheDb, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::JourneyVector,
    main_db::{self, MainDb},
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
// Returns a journey bitmap for the journey kind
fn get_range_internal(
    txn: &main_db::Txn,
    from_date_inclusive: Option<NaiveDate>,
    to_date_inclusive: Option<NaiveDate>,
    kind: Option<&JourneyKind>, // TODO: dpending on the future design, we might not want this to be optional.
) -> Result<JourneyBitmap> {
    let mut journey_map = JourneyBitmap::new();

    for journey_header in txn.query_journeys(from_date_inclusive, to_date_inclusive)? {
        let journey_kind = journey_header.journey_kind;

        let should_include = match kind {
            None => true,
            Some(kind) => *kind == journey_kind,
        };

        if should_include {
            let journey_data = txn.get_journey_data(&journey_header.id)?;
            match journey_data {
                JourneyData::Bitmap(bitmap) => journey_map.merge(bitmap),
                JourneyData::Vector(vector) => {
                    add_journey_vector_to_journey_bitmap(&mut journey_map, &vector);
                }
            }
        }
    }

    Ok(journey_map)
}

// for time machine
pub fn get_range(
    txn: &mut main_db::Txn,
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
    kind: Option<&JourneyKind>,
) -> Result<JourneyBitmap> {
    get_range_internal(
        txn,
        Some(from_date_inclusive),
        Some(to_date_inclusive),
        kind,
    )
}

fn get_all_finalized_journeys(
    main_db_txn: &main_db::Txn,
    cache_db: &CacheDb,
    layer_kind: &LayerKind,
) -> Result<JourneyBitmap> {
    cache_db.get_full_journey_cache_or_compute(layer_kind, || match layer_kind {
        LayerKind::All => {
            let mut default_bitmap = get_all_finalized_journeys(
                main_db_txn,
                cache_db,
                &LayerKind::JounreyKind(JourneyKind::DefaultKind),
            )?;
            let flight_bitmap = get_all_finalized_journeys(
                main_db_txn,
                cache_db,
                &LayerKind::JounreyKind(JourneyKind::Flight),
            )?;
            default_bitmap.merge(flight_bitmap);
            Ok(default_bitmap)
        }
        LayerKind::JounreyKind(kind) => get_range_internal(main_db_txn, None, None, Some(kind)),
    })
}

// main map
pub fn get_latest_including_ongoing(
    main_db: &mut MainDb,
    cache_db: &CacheDb,
    layer_kind: &LayerKind,
) -> Result<JourneyBitmap> {
    main_db.with_txn(|txn| {
        let mut journey_bitmap = get_all_finalized_journeys(txn, cache_db, layer_kind)?;

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
