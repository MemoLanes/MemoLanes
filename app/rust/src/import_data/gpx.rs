use crate::api::import::ImportPreprocessor;
use crate::gps_processor::Point;
use crate::gpx_file_utils::analyze_and_prepare_gpx;
use crate::raw_data::RawGPSPoint;
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Utc};
use gpx::{read, Waypoint};
use std::fs;

#[auto_context]
pub fn load_gpx(file_path: &str) -> Result<(Vec<Vec<RawGPSPoint>>, ImportPreprocessor)> {
    // TODO: it is pretty inefficient to read the whole file into memory first.
    // Some of the GPX files can be very large. Probably we want streaming.
    let xml = fs::read_to_string(file_path)?;
    let (xml, preprocessor) = analyze_and_prepare_gpx(&xml)?;
    let gpx = read(xml.as_bytes())?;
    let raw_data = load_gpx_raw_data(&gpx)?;
    Ok((raw_data, preprocessor))
}

pub fn load_gpx_raw_data(gpx_data: &gpx::Gpx) -> Result<Vec<Vec<RawGPSPoint>>> {
    let convert_to_timestamp = |time: &Option<gpx::Time>| -> Result<Option<i64>> {
        match time {
            Some(t) => {
                let s = t.format()?;
                let dt = DateTime::<Utc>::from(DateTime::parse_from_rfc3339(&s)?);
                Ok(Some(dt.timestamp_millis()))
            }
            None => Ok(None),
        }
    };

    let waypoint_to_rawdata = |point: &Waypoint| -> Result<RawGPSPoint> {
        Ok(RawGPSPoint {
            point: Point {
                latitude: point.point().y(),
                longitude: point.point().x(),
            },
            timestamp_ms: convert_to_timestamp(&point.time)?,
            accuracy: point.hdop.map(|v| v as f32),
            altitude: point.elevation.map(|v| v as f32),
            speed: point.speed.map(|v| v as f32),
        })
    };

    let track_data = gpx_data
        .tracks
        .iter()
        .flat_map(|t| t.segments.iter())
        .map(|s| {
            s.points
                .iter()
                .map(waypoint_to_rawdata)
                .collect::<Result<Vec<_>>>()
        })
        .filter_map(Result::ok)
        .filter(|v| !v.is_empty());

    let route_data = gpx_data
        .routes
        .iter()
        .map(|r| {
            r.points
                .iter()
                .map(waypoint_to_rawdata)
                .collect::<Result<Vec<_>>>()
        })
        .filter_map(Result::ok)
        .filter(|v| !v.is_empty());

    Ok(track_data.chain(route_data).collect())
}
