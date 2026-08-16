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
    let journey_kind = match layer_kind {
        LayerKind::All => None,
        LayerKind::JourneyKind(kind) => Some(*kind),
    };
    for journey_id in txn.query_journey_ids_in_date_range(from, to, journey_kind)? {
        let data = txn.get_journey_data(&journey_id)?;
        data.merge_into(&mut bitmap);
    }
    Ok(bitmap)
}
