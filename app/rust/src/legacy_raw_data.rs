//! Access to raw GPS data captured by the legacy CSV-on-filesystem format.
//!
//! Raw data v2 is attached to journeys in the main database and is implemented
//! by [`crate::raw_data`]. This module exists only to keep old CSV files
//! accessible and exportable.

use std::{
    fs::{remove_file, File},
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::Path,
};

use anyhow::{Context, Result};
use auto_context::auto_context;
use csv::Reader;
use gpx::TrackSegment;
use serde::Deserialize;

use crate::export_data::gpx::{raw_data_waypoint, write_gpx_with_segments, RAW_DATA_TYPE_NAME};

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
    #[allow(dead_code)]
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

#[auto_context]
pub fn legacy_raw_data_csv_to_gpx_file<R: Read, W: Write + Seek>(
    csv_reader: &mut Reader<R>,
    writer: &mut W,
) -> Result<()> {
    let mut segment = TrackSegment { points: Vec::new() };
    for result in csv_reader.deserialize::<LegacyRawDataCsvRow>() {
        let raw = result?;
        segment.points.push(raw_data_waypoint(
            raw.latitude,
            raw.longitude,
            raw.timestamp_ms.or(Some(raw.received_timestamp_ms)),
            raw.altitude,
            raw.accuracy,
        )?);
    }
    write_gpx_with_segments(vec![segment], Some(RAW_DATA_TYPE_NAME), writer)
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
    let gpx_file = File::create(&gpx_path)
        .with_context(|| format!("Failed to create legacy GPX file: {gpx_path_string}"))?;
    let mut writer = BufWriter::new(gpx_file);

    legacy_raw_data_csv_to_gpx_file(&mut reader, &mut writer)
        .with_context(|| format!("Failed to convert legacy CSV to GPX: {csv_filepath}"))?;
    Ok(gpx_path_string)
}
