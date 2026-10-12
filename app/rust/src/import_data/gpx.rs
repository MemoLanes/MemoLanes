use crate::gps::{Point, RawGPSPoint};
use crate::gpx_file_utils::{analyze_and_prepare_gpx, MEMOLANES_NAMESPACE};
use crate::import_data::{ImportMode, ImportPreprocessor, ImportedJourney, ParsedVectorData};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, NaiveDate, Utc};
use gpx::{read, Waypoint};
use log::warn;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use std::fs;

/// Parses a GPX file while retaining MemoLanes Journey and segment boundaries.
#[auto_context]
pub fn load_gpx(file_path: &str) -> Result<(ParsedVectorData, ImportPreprocessor)> {
    let xml = fs::read_to_string(file_path)
        .with_context(|| format!("Could not read GPX file: {file_path}"))?;
    if xml.contains(MEMOLANES_NAMESPACE) {
        if let Some(parsed) = parse_memolanes_gpx(&xml)? {
            return Ok((parsed, ImportPreprocessor::None));
        }
    }

    // The gpx crate handles third-party waypoint attributes, times and routes.
    let (xml, preprocessor) = analyze_and_prepare_gpx(&xml)?;
    let gpx = read(xml.as_bytes()).context("Could not parse GPX tracks and routes")?;
    let groups = load_gpx_raw_data(&gpx)?
        .into_iter()
        .map(|segment| ImportedJourney::generic(vec![segment]))
        .collect();
    Ok((
        ParsedVectorData::new(ImportMode::GenericByDate, groups)?,
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

fn local_name(name: &str) -> &str {
    name.rsplit_once(':')
        .map(|(_, local)| local)
        .unwrap_or(name)
}

fn attribute(event: &BytesStart<'_>, name: &str) -> Result<Option<String>> {
    for attribute in event.attributes() {
        let attribute = attribute.context("Invalid GPX XML attribute")?;
        if attribute.key.as_ref() == name {
            return Ok(Some(
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

fn metadata(event: &BytesStart<'_>) -> Result<Option<ImportedJourney>> {
    let date = attribute(event, "date")?;
    let journey_date = date
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
    if journey_date.is_none() {
        warn!("Invalid MemoLanes GPX journey date {date:?}; using generic GPX import");
        return Ok(None);
    }

    let optional_time = |name: &str| -> Result<Option<DateTime<Utc>>> {
        let Some(value) = attribute(event, name)? else {
            return Ok(None);
        };
        match DateTime::parse_from_rfc3339(&value) {
            Ok(value) => Ok(Some(value.with_timezone(&Utc))),
            Err(error) => {
                warn!("Invalid MemoLanes GPX {} timestamp: {error}", name);
                Ok(None)
            }
        }
    };

    Ok(Some(ImportedJourney {
        journey_date,
        start: optional_time("start")?,
        end: optional_time("end")?,
        segments: Vec::new(),
    }))
}

fn track_point(event: &BytesStart<'_>) -> Result<RawGPSPoint> {
    let latitude: f64 = attribute(event, "lat")?
        .context("MemoLanes GPX track point has no latitude")?
        .parse()
        .context("MemoLanes GPX track point has invalid latitude")?;
    let longitude: f64 = attribute(event, "lon")?
        .context("MemoLanes GPX track point has no longitude")?
        .parse()
        .context("MemoLanes GPX track point has invalid longitude")?;
    if !latitude.is_finite() || !longitude.is_finite() {
        anyhow::bail!("MemoLanes GPX track point has non-finite coordinates");
    }
    Ok(RawGPSPoint {
        point: Point {
            latitude,
            longitude,
        },
        timestamp_ms: None,
        accuracy: None,
        altitude: None,
        speed: None,
    })
}

/// Parse geometry and metadata together, without joining parser results by track index.
fn parse_memolanes_gpx(xml: &str) -> Result<Option<ParsedVectorData>> {
    let mut reader = NsReader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut groups = Vec::new();
    let mut current: Option<ImportedJourney> = None;
    let mut segment: Option<Vec<RawGPSPoint>> = None;
    let mut marked = false;
    let mut invalid_marker = false;
    let mut has_routes = false;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .context("Invalid MemoLanes GPX XML")?;
        let is_gpx = matches!(&namespace, ResolveResult::Unbound)
            || matches!(&namespace, ResolveResult::Bound(uri)
                if matches!(uri.as_ref(), "http://www.topografix.com/GPX/1/1" | "http://www.topografix.com/GPX/1/0"));
        let empty = matches!(&event, Event::Empty(_));
        let closing = match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = local_name(element.name().as_ref()).to_owned();
                match name.as_str() {
                    "rte" if is_gpx && path == ["gpx"] => has_routes = true,
                    "trk" if is_gpx && path == ["gpx"] => {
                        current = Some(ImportedJourney::generic(Vec::new()));
                    }
                    "journey"
                        if path == ["gpx", "trk", "extensions"]
                            && matches!(namespace, ResolveResult::Bound(uri) if uri.as_ref() == MEMOLANES_NAMESPACE) =>
                    {
                        marked = true;
                        match metadata(&element) {
                            Ok(Some(parsed)) => {
                                if let Some(group) = current.as_mut() {
                                    group.journey_date = parsed.journey_date;
                                    group.start = parsed.start;
                                    group.end = parsed.end;
                                }
                            }
                            Ok(None) => invalid_marker = true,
                            Err(error) => {
                                warn!("Invalid MemoLanes GPX journey metadata: {error:#}; using generic GPX import");
                                invalid_marker = true;
                            }
                        }
                    }
                    "trkseg" if is_gpx && path == ["gpx", "trk"] => {
                        segment = Some(Vec::new());
                    }
                    "trkpt" if is_gpx && path == ["gpx", "trk", "trkseg"] => {
                        if let Some(points) = segment.as_mut() {
                            points.push(track_point(&element)?);
                        }
                    }
                    _ => {}
                }
                if empty {
                    Some(name)
                } else {
                    // A foreign ancestor cannot form part of a GPX structural path.
                    path.push(if is_gpx { name } else { String::new() });
                    None
                }
            }
            Event::End(element) => {
                path.pop();
                Some(local_name(element.name().as_ref()).to_owned())
            }
            Event::Eof => break,
            _ => None,
        };
        if is_gpx {
            match closing.as_deref() {
                Some("trkseg") if path == ["gpx", "trk"] => {
                    if let (Some(group), Some(points)) = (current.as_mut(), segment.take()) {
                        group.segments.push(points);
                    }
                }
                Some("trk") if path == ["gpx"] => {
                    if let Some(group) = current.take() {
                        groups.push(group);
                    }
                }
                _ => {}
            }
        }
    }

    if invalid_marker || !marked {
        if !marked {
            warn!("MemoLanes GPX namespace found without valid journey metadata; using generic GPX import");
        }
        return Ok(None);
    }
    if has_routes || groups.iter().any(|group| !group.has_memolanes_metadata()) {
        warn!("MemoLanes GPX contains unmarked tracks or routes; using generic GPX import");
        return Ok(None);
    }
    Ok(Some(ParsedVectorData::new(
        ImportMode::ExportedJourney,
        groups,
    )?))
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
