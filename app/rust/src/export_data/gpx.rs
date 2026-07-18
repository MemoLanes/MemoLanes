use crate::journey_vector::JourneyVector;
use crate::raw_data::JourneyRawData;
use crate::storage::RawCsvRow;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use csv::Reader;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Metadata, Track, TrackSegment, Waypoint};
use std::io::{Seek, Write};
use time::OffsetDateTime;

// TODO: Pull in more metadata to the exported files, e.g. timestamp, note, etc
// For most things, we could put them as custom attributes. The timestamp is a
// bit annoying. Ideally I don't want to fake data (e.g. generating timestamps
// for all points based on begin and end time). So maybe also treat them as
// custom attributes or just add timestamp for the first and last point if possible.
fn write_gpx_with_segments<T: Write + Seek>(
    segments: Vec<TrackSegment>,
    name: Option<&str>,
    writer: &mut T,
) -> Result<()> {
    if segments.is_empty() {
        anyhow::bail!("No track segments");
    }

    let track = Track {
        name: Some("MemoLanes Track".to_string()),
        comment: None,
        description: None,
        source: None,
        links: vec![],
        type_: None,
        number: None,
        segments,
    };

    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        creator: Some("MemoLanes".to_string()),
        metadata: Some(Metadata {
            name: name.map(str::to_string),
            ..Default::default()
        }),
        waypoints: vec![],
        tracks: vec![track],
        routes: vec![],
    };

    gpx::write(&gpx, writer)?;
    Ok(())
}

pub const JOURNEY_TYPE_NAME: &str = "MemoLanes Journey";
pub const RAWDATA_TYPE_NAME: &str = "MemoLanes RawData";

#[auto_context]
pub fn journey_vector_to_gpx_file<T: Write + Seek>(
    journey_vector: &JourneyVector,
    writer: &mut T,
) -> Result<()> {
    let mut segments = Vec::new();

    for track_segment in &journey_vector.track_segments {
        let mut points = Vec::new();
        track_segment.track_points.iter().for_each(|point| {
            points.push(Waypoint::new(Point::new(point.longitude, point.latitude)));
        });
        segments.push(TrackSegment { points });
    }
    write_gpx_with_segments(segments, Some(JOURNEY_TYPE_NAME), writer)
}

#[auto_context]
pub fn raw_data_csv_to_gpx_file<R: std::io::Read, W: Write + Seek>(
    csv_reader: &mut Reader<R>,
    writer: &mut W,
) -> Result<()> {
    let mut segment = TrackSegment { points: Vec::new() };

    for result in csv_reader.deserialize::<RawCsvRow>() {
        let raw: RawCsvRow = result?;
        segment.points.push(raw_waypoint(
            raw.latitude,
            raw.longitude,
            raw.timestamp_ms.or(Some(raw.received_timestamp_ms)),
            raw.altitude,
            raw.accuracy,
        )?);
    }
    write_gpx_with_segments(vec![segment], Some(RAWDATA_TYPE_NAME), writer)
}

#[auto_context]
pub fn journey_raw_data_to_gpx_file<W: Write + Seek>(
    raw_data: &JourneyRawData,
    writer: &mut W,
) -> Result<()> {
    let points = raw_data
        .points
        .iter()
        .map(|point| {
            let raw = &point.raw_gps_point;
            raw_waypoint(
                raw.point.latitude,
                raw.point.longitude,
                raw.timestamp_ms.or(Some(point.received_timestamp_ms)),
                raw.altitude,
                raw.accuracy,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    write_gpx_with_segments(
        vec![TrackSegment { points }],
        Some(RAWDATA_TYPE_NAME),
        writer,
    )
}

fn raw_waypoint(
    latitude: f64,
    longitude: f64,
    timestamp_ms: Option<i64>,
    altitude: Option<f32>,
    accuracy: Option<f32>,
) -> Result<Waypoint> {
    let mut waypoint = Waypoint::new(Point::new(longitude, latitude));
    if let Some(timestamp_ms) = timestamp_ms.filter(|timestamp_ms| *timestamp_ms > 0) {
        let time = OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_ms) * 1_000_000)?;
        waypoint.time = Some(time.into());
    }
    waypoint.elevation = altitude.map(f64::from);
    waypoint.hdop = accuracy.map(f64::from);
    Ok(waypoint)
}
