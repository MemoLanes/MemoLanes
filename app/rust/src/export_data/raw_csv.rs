//! CSV export for journey-attached raw data v2.

use std::io::Write;

use anyhow::{Context, Result};
use auto_context::auto_context;
use serde::Serialize;

use crate::raw_data::{JourneyRawData, RawGPSPoint};

#[derive(Serialize)]
struct JourneyRawDataCsvRow {
    timestamp_ms: Option<i64>,
    received_timestamp_ms: i64,
    latitude: f64,
    longitude: f64,
    accuracy: Option<f32>,
    altitude: Option<f32>,
    speed: Option<f32>,
}

impl JourneyRawDataCsvRow {
    fn new(raw_gps_point: &RawGPSPoint, received_timestamp_ms: i64) -> Self {
        Self {
            timestamp_ms: raw_gps_point.timestamp_ms,
            received_timestamp_ms,
            latitude: raw_gps_point.point.latitude,
            longitude: raw_gps_point.point.longitude,
            accuracy: raw_gps_point.accuracy,
            altitude: raw_gps_point.altitude,
            speed: raw_gps_point.speed,
        }
    }
}

#[auto_context]
pub fn journey_raw_data_to_csv_file<W: Write>(
    raw_data: &JourneyRawData,
    writer: &mut W,
) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(writer);
    for point in &raw_data.points {
        writer.serialize(JourneyRawDataCsvRow::new(
            &point.raw_gps_point,
            point.received_timestamp_ms,
        ))?;
    }
    writer.flush()?;
    Ok(())
}
