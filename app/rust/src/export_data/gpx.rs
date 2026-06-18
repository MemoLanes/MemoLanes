use crate::journey_vector::JourneyVector;
use crate::storage::RawCsvRow;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use csv::Reader;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Metadata, Track, TrackSegment, Waypoint};
use std::io::{Seek, Write};
use time::{Duration, OffsetDateTime};

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

        let mut wp = Waypoint::new(Point::new(raw.longitude, raw.latitude));

        if let Some(ts) = raw.timestamp_ms {
            if ts > 0 {
                let dt = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(ts);
                wp.time = Some(dt.into());
            }
        }

        if let Some(alt) = raw.altitude {
            wp.elevation = Some(alt as f64);
        }

        if let Some(acc) = raw.accuracy {
            wp.hdop = Some(acc as f64);
        }

        segment.points.push(wp);
    }
    write_gpx_with_segments(vec![segment], Some(RAWDATA_TYPE_NAME), writer)
}
