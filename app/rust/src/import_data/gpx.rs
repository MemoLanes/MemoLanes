use crate::gps::{Point, RawGPSPoint};
use crate::gpx_file_utils::analyze_and_prepare_gpx;
use crate::import_data::{ImportMode, ImportPreprocessor, ImportedJourney, ParsedVectorData};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Utc};
use gpx::{read, Waypoint};
use log::warn;
use std::fs;

/// Parses GPX tracks and routes, retaining their segment boundaries.
#[auto_context]
pub fn load_gpx(file_path: &str) -> Result<(ParsedVectorData, ImportPreprocessor)> {
    let xml = fs::read_to_string(file_path)
        .with_context(|| format!("Could not read GPX file: {file_path}"))?;
    // The gpx crate handles third-party waypoint attributes, times and routes.
    let (xml, preprocessor) = analyze_and_prepare_gpx(&xml)?;
    let gpx = read(xml.as_bytes()).context("Could not parse GPX tracks and routes")?;
    let groups = load_gpx_raw_data(&gpx)?
        .into_iter()
        .map(|segment| ImportedJourney::generic(vec![segment]))
        .collect();
    Ok((
        ParsedVectorData::new(ImportMode::GenericByDate, groups),
        preprocessor,
    ))
}

fn waypoint_to_rawdata(point: &Waypoint) -> Result<RawGPSPoint> {
    let timestamp_ms = match &point.time {
        Some(time) => {
            let value = time.format()?;
            Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(&value)?).timestamp_millis())
        }
        None => None,
    };
    Ok(RawGPSPoint {
        point: Point {
            latitude: point.point().y(),
            longitude: point.point().x(),
        },
        timestamp_ms,
        accuracy: point.hdop.map(|value| value as f32),
        altitude: point.elevation.map(|value| value as f32),
        speed: point.speed.map(|value| value as f32),
    })
}

pub fn load_gpx_raw_data(gpx_data: &gpx::Gpx) -> Result<Vec<Vec<RawGPSPoint>>> {
    let track_data = gpx_data
        .tracks
        .iter()
        .flat_map(|track| track.segments.iter())
        .map(|segment| {
            segment
                .points
                .iter()
                .map(waypoint_to_rawdata)
                .collect::<Result<Vec<_>>>()
        })
        .filter_map(|result| match result {
            Ok(segment) => Some(segment),
            Err(error) => {
                warn!("Skipping invalid third-party GPX track segment: {error:#}");
                None
            }
        })
        .filter(|segment| !segment.is_empty());

    let route_data = gpx_data
        .routes
        .iter()
        .map(|route| {
            route
                .points
                .iter()
                .map(waypoint_to_rawdata)
                .collect::<Result<Vec<_>>>()
        })
        .filter_map(|result| match result {
            Ok(segment) => Some(segment),
            Err(error) => {
                warn!("Skipping invalid third-party GPX route: {error:#}");
                None
            }
        })
        .filter(|segment| !segment.is_empty());

    Ok(track_data.chain(route_data).collect())
}
