//! CSV export for journey-attached raw data v2.

use std::io::Write;

use anyhow::{Context, Result};
use auto_context::auto_context;
use serde::Serialize;

use crate::raw_data::JourneyRawData;

#[derive(Serialize)]
struct JourneyRawDataCsvRow<'a> {
    timestamp_ms: Option<i64>,
    received_timestamp_ms: i64,
    latitude: f64,
    longitude: f64,
    accuracy: Option<f32>,
    altitude: Option<f32>,
    speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    journey_id: Option<&'a str>,
}

pub(super) fn write_raw_data_csv<W: Write>(
    writer: &mut csv::Writer<W>,
    raw_data: &JourneyRawData,
    journey_id: Option<&str>,
) -> Result<()> {
    for point in &raw_data.points {
        let gps = &point.raw_gps_point;
        writer.serialize(JourneyRawDataCsvRow {
            timestamp_ms: gps.timestamp_ms,
            received_timestamp_ms: point.received_timestamp_ms,
            latitude: gps.point.latitude,
            longitude: gps.point.longitude,
            accuracy: gps.accuracy,
            altitude: gps.altitude,
            speed: gps.speed,
            journey_id,
        })?;
    }
    Ok(())
}

#[auto_context]
pub fn journey_raw_data_to_csv_file<W: Write>(
    raw_data: &JourneyRawData,
    writer: &mut W,
) -> Result<()> {
    let mut writer = csv::Writer::from_writer(writer);
    write_raw_data_csv(&mut writer, raw_data, None)?;
    writer.flush()?;
    Ok(())
}
