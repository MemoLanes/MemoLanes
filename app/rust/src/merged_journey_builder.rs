/* We store journey one by one, but for a lot of use cases such as rendering, we
need to merge all journeys into one `journey_bitmap`. Relavent functionailties is
implemented here.
*/
use crate::{
    cache_db::{CacheDb, LayerKind},
    journey_bitmap::JourneyBitmap,
    journey_header::JourneyKind,
    main_db,
};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::NaiveDate;

// for time machine
#[auto_context]
pub fn get_range(
    txn: &mut main_db::Txn,
    cache_db: &dyn CacheDb,
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
    kind: Option<&JourneyKind>,
) -> Result<JourneyBitmap> {
    let layer_kind = match kind {
        Some(k) => LayerKind::JourneyKind(*k),
        None => LayerKind::All,
    };
    cache_db.get_or_compute(
        txn,
        &layer_kind,
        Some(from_date_inclusive),
        Some(to_date_inclusive),
    )
}

// main map
#[auto_context]
pub fn get_full(
    txn: &main_db::Txn,
    cache_db: &dyn CacheDb,
    layer_kind: &Option<LayerKind>,
    include_ongoing: bool,
) -> Result<JourneyBitmap> {
    let mut journey_bitmap = match layer_kind {
        Some(layer_kind) => cache_db.get_or_compute(txn, layer_kind, None, None)?,
        None => JourneyBitmap::new(),
    };

    if include_ongoing {
        if let Some(journey_vector) = txn.get_ongoing_journey(None)? {
            journey_bitmap.merge_vector(&journey_vector);
        }
    }

    // NOTE: Calling to `main_db.with_txn` directly without going through
    // `storage` is fine here because we are not modifying main db here.
    // But just to make sure:
    assert_eq!(txn.action, None);
    Ok(journey_bitmap)
}
