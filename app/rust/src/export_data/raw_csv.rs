use std::io::Write;

use anyhow::{Context, Result};
use auto_context::auto_context;

use crate::{raw_data::JourneyRawData, storage::RawCsvRow};

#[auto_context]
pub fn journey_raw_data_to_csv_file<W: Write>(
    raw_data: &JourneyRawData,
    writer: &mut W,
) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(writer);
    for point in &raw_data.points {
        writer.serialize(RawCsvRow::create_from_raw_data(
            &point.raw_gps_point,
            point.received_timestamp_ms,
        ))?;
    }
    writer.flush()?;
    Ok(())
}
