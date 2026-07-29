use std::{fs::File, io::Read};

use anyhow::{Context, Result};
use csv::{Reader, ReaderBuilder, StringRecord, Trim};
use serde::Deserialize;

use crate::{
    api::import::ImportPreprocessor,
    gps_processor::{Point, RawData},
};

const DOL_HEADERS: &[&str] = &[
    "timestamp",
    "longitude",
    "latitude",
    "heading",
    "accuracy",
    "verticalAccuracy",
    "speed",
    "distance",
    "altitude",
    "course",
    "param1",
    "param2",
    "param3",
];

#[derive(Deserialize)]
struct DolRow {
    timestamp: i64,
    longitude: f64,
    latitude: f64,
    #[serde(default)]
    accuracy: Option<f32>,
    #[serde(default)]
    speed: Option<f32>,
    #[serde(default)]
    altitude: Option<f32>,
}

fn is_dol_format(headers: &StringRecord) -> bool {
    headers.iter().eq(DOL_HEADERS.iter().copied())
}

fn parse_dol_rows<R: Read>(reader: &mut Reader<R>) -> Result<Vec<RawData>> {
    let mut raw_data = Vec::new();

    for (index, result) in reader.deserialize::<DolRow>().enumerate() {
        let row_number = index + 2;
        let row = result.with_context(|| format!("failed to parse CSV row {row_number}"))?;

        if !row.latitude.is_finite() || !(-90.0..=90.0).contains(&row.latitude) {
            anyhow::bail!("invalid latitude {} at CSV row {row_number}", row.latitude);
        }
        if !row.longitude.is_finite() || !(-180.0..=180.0).contains(&row.longitude) {
            anyhow::bail!(
                "invalid longitude {} at CSV row {row_number}",
                row.longitude
            );
        }

        let timestamp_ms = row
            .timestamp
            .checked_mul(1000)
            .with_context(|| format!("timestamp overflow at CSV row {row_number}"))?;

        raw_data.push(RawData {
            point: Point {
                latitude: row.latitude,
                longitude: row.longitude,
            },
            timestamp_ms: Some(timestamp_ms),
            accuracy: non_negative_finite(row.accuracy),
            altitude: finite(row.altitude),
            speed: non_negative_finite(row.speed),
        });
    }

    Ok(raw_data)
}

fn finite(value: Option<f32>) -> Option<f32> {
    value.filter(|value| value.is_finite())
}

fn non_negative_finite(value: Option<f32>) -> Option<f32> {
    finite(value).filter(|value| *value >= 0.0)
}

pub fn load_csv(file_path: &str) -> Result<(Vec<Vec<RawData>>, ImportPreprocessor)> {
    let file =
        File::open(file_path).with_context(|| format!("failed to open CSV file: {file_path}"))?;
    let mut reader = ReaderBuilder::new().trim(Trim::All).from_reader(file);
    let headers = reader
        .headers()
        .with_context(|| format!("failed to read CSV headers: {file_path}"))?
        .clone();

    let (raw_data, preprocessor) = if is_dol_format(&headers) {
        (
            parse_dol_rows(&mut reader).context("failed to parse DoL CSV")?,
            ImportPreprocessor::Spare,
        )
    } else {
        anyhow::bail!(
            "unsupported CSV format; headers: {}",
            headers.iter().collect::<Vec<_>>().join(",")
        );
    };

    if raw_data.is_empty() {
        anyhow::bail!("CSV file contains no data rows");
    }

    Ok((vec![raw_data], preprocessor))
}
