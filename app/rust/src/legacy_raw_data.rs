//! Access to raw GPS data captured by the legacy CSV-on-filesystem format.
//!
//! Raw data v2 is attached to journeys in the main database and is implemented
//! by [`crate::raw_data`]. This module exists only to keep old CSV files
//! accessible and exportable.

use std::{
    fs::{remove_file, File},
    io::{BufReader, Read, Write},
    path::Path,
};

use anyhow::{Context, Result};
use auto_context::auto_context;
use csv::Reader;
use serde::Deserialize;

use crate::export_data::{gpx::GpxJourneyWriter, write_export_file};
use crate::gps::{ExtendedRawGPSPoint, Point, RawGPSPoint};

const LEGACY_RAW_DATA_DIR_NAME: &str = "raw_data";

pub struct LegacyRawDataFile {
    pub name: String,
    pub path: String,
}

fn legacy_raw_data_file_from_entry(
    entry_result: std::io::Result<std::fs::DirEntry>,
) -> Option<LegacyRawDataFile> {
    let path = entry_result.ok()?.path();
    if !path.is_file() || path.extension().and_then(|extension| extension.to_str()) != Some("csv") {
        return None;
    }
    Some(LegacyRawDataFile {
        name: path.file_stem()?.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
    })
}

#[derive(Debug, Deserialize)]
struct LegacyRawDataCsvRow {
    timestamp_ms: Option<i64>,
    received_timestamp_ms: i64,
    latitude: f64,
    longitude: f64,
    accuracy: Option<f32>,
    altitude: Option<f32>,
    speed: Option<f32>,
}

#[auto_context]
pub fn delete_legacy_raw_data_file(support_dir: &str, filename: &str) -> Result<()> {
    let filename = if Path::new(filename).extension().is_some() {
        filename.to_owned()
    } else {
        format!("{filename}.csv")
    };
    let path = Path::new(support_dir)
        .join(LEGACY_RAW_DATA_DIR_NAME)
        .join(filename);

    remove_file(&path)
        .with_context(|| format!("failed to remove legacy raw data file: {}", path.display()))
}

#[auto_context]
pub fn list_all_legacy_raw_data(support_dir: &str) -> Result<Vec<LegacyRawDataFile>> {
    let dir = Path::new(support_dir).join(LEGACY_RAW_DATA_DIR_NAME);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        anyhow::bail!("legacy raw data path exists but is not a directory: {dir:?}");
    }

    let mut result: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(legacy_raw_data_file_from_entry)
        .collect();
    result.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(result)
}

// TODO: Retained for RawDataPage's legacy CSV-to-GPX export (raw_data_page.dart).
#[auto_context]
pub fn legacy_raw_data_csv_to_gpx_file<R: Read, W: Write>(
    csv_reader: &mut Reader<R>,
    writer: &mut W,
) -> Result<()> {
    let points = csv_reader.deserialize::<LegacyRawDataCsvRow>().map(|row| {
        let raw = row?;
        Ok(ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                point: Point {
                    latitude: raw.latitude,
                    longitude: raw.longitude,
                },
                timestamp_ms: raw.timestamp_ms,
                altitude: raw.altitude,
                accuracy: raw.accuracy,
                speed: raw.speed,
            },
            received_timestamp_ms: raw.received_timestamp_ms,
        })
    });
    let mut writer = GpxJourneyWriter::new_raw_data(writer)?;
    writer.write_raw_data_points(points, "MemoLanes Track")?;
    writer.finish()
}

#[auto_context]
pub fn export_legacy_raw_data_gpx_file(csv_filepath: &str, cache_dir: &str) -> Result<String> {
    let csv_path = Path::new(csv_filepath);
    let file_name = csv_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse filename: {csv_filepath}"))?;

    let target_dir = Path::new(cache_dir).join(LEGACY_RAW_DATA_DIR_NAME);
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    let gpx_path = target_dir.join(file_name).with_extension("gpx");
    let gpx_path_string = gpx_path.to_string_lossy().into_owned();
    if gpx_path.exists() {
        return Ok(gpx_path_string);
    }

    let csv_file = File::open(csv_path)
        .with_context(|| format!("Failed to open legacy CSV file: {csv_filepath}"))?;
    let mut reader = Reader::from_reader(BufReader::new(csv_file));
    write_export_file(&gpx_path, |writer| {
        legacy_raw_data_csv_to_gpx_file(&mut reader, writer)
            .with_context(|| format!("Failed to convert legacy CSV to GPX: {csv_filepath}"))
    })?;
    Ok(gpx_path_string)
}
