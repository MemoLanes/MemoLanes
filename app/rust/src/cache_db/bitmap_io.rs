use anyhow::Result;
use rusqlite::Connection;

use crate::journey_bitmap::JourneyBitmap;

pub(super) fn read_bitmap(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Option<JourneyBitmap>> {
    use rusqlite::OptionalExtension;

    let mut stmt = conn.prepare(sql)?;
    stmt.query_row(params, |row| {
        let data = row.get_ref(0)?.as_blob()?;
        Ok(crate::journey_data::deserialize_journey_bitmap(data, false))
    })
    .optional()?
    .transpose()
}

pub(super) fn to_blob(bitmap: &mut JourneyBitmap) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    crate::journey_data::serialize_journey_bitmap(bitmap, &mut data)?;
    Ok(data)
}
