use std::{fs::File, io::Read};

use anyhow::{Context, Result};
use csv::{Reader, ReaderBuilder, StringRecord, Trim};
use serde::Deserialize;

use crate::{
    api::import::ImportPreprocessor,
    gps_processor::{Point, RawData},
};

// CSV exported by 人生点点
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

// CSV exported by 一生足迹
const STEP_HEADERS: &[&str] = &[
    "dataTime",
    "locType",
    "longitude",
    "latitude",
    "heading",
    "accuracy",
    "speed",
    "distance",
    "isBackForeground",
    "stepType",
    "altitude",
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

#[derive(Deserialize)]
struct StepRow {
    #[serde(rename = "dataTime")]
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

fn is_step_format(headers: &StringRecord) -> bool {
    headers.iter().eq(STEP_HEADERS.iter().copied())
}

fn parse_dol_rows<R: Read>(reader: &mut Reader<R>) -> Result<Vec<RawData>> {
    let mut raw_data = Vec::new();

    for (index, result) in reader.deserialize::<DolRow>().enumerate() {
        let row_number = index + 2;
        let row = result.with_context(|| format!("failed to parse CSV row {row_number}"))?;
        raw_data.push(raw_data_from_values(
            row.timestamp,
            row.longitude,
            row.latitude,
            row.accuracy,
            row.altitude,
            row.speed,
            row_number,
        )?);
    }

    Ok(raw_data)
}

fn parse_step_rows<R: Read>(reader: &mut Reader<R>) -> Result<Vec<RawData>> {
    let mut raw_data = Vec::new();

    for (index, result) in reader.deserialize::<StepRow>().enumerate() {
        let row_number = index + 2;
        let row = result.with_context(|| format!("failed to parse CSV row {row_number}"))?;
        raw_data.push(raw_data_from_values(
            row.timestamp,
            row.longitude,
            row.latitude,
            row.accuracy,
            row.altitude,
            row.speed,
            row_number,
        )?);
    }

    Ok(raw_data)
}

fn raw_data_from_values(
    timestamp: i64,
    longitude: f64,
    latitude: f64,
    accuracy: Option<f32>,
    altitude: Option<f32>,
    speed: Option<f32>,
    row_number: usize,
) -> Result<RawData> {
    if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
        anyhow::bail!("invalid latitude {latitude} at CSV row {row_number}");
    }
    if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
        anyhow::bail!("invalid longitude {longitude} at CSV row {row_number}");
    }

    let timestamp_ms = timestamp
        .checked_mul(1000)
        .with_context(|| format!("timestamp overflow at CSV row {row_number}"))?;

    Ok(RawData {
        point: Point {
            latitude,
            longitude,
        },
        timestamp_ms: Some(timestamp_ms),
        accuracy: non_negative_finite(accuracy),
        altitude: finite(altitude),
        speed: non_negative_finite(speed),
    })
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
    } else if is_step_format(&headers) {
        (
            parse_step_rows(&mut reader).context("failed to parse Step CSV")?,
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
