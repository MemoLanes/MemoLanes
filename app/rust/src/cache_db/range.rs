use anyhow::Result;
use chrono::NaiveDate;

use super::LayerKind;
use crate::{journey_bitmap::JourneyBitmap, main_db};

pub fn compute(
    txn: &main_db::Txn,
    from: NaiveDate,
    to: NaiveDate,
    layer_kind: &LayerKind,
) -> Result<JourneyBitmap> {
    let mut bitmap = JourneyBitmap::new();
    for header in txn.query_journeys(Some(from), Some(to))? {
        let include = match layer_kind {
            LayerKind::All => true,
            LayerKind::JourneyKind(kind) => *kind == header.journey_kind,
        };
        if include {
            let data = txn.get_journey_data(&header.id)?;
            data.merge_into(&mut bitmap);
        }
    }
    Ok(bitmap)
}
