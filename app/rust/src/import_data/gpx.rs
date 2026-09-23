use crate::api::import::ImportPreprocessor;
use crate::gps_processor::{Point, RawData};
use crate::gpx_file_utils::{analyze_and_prepare_gpx, MEMOLANES_NAMESPACE};
use crate::import_data::{ImportedJourney, ParsedVectorData};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, NaiveDate, Utc};
use gpx::{read, Waypoint};
use log::warn;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
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
    Ok((ParsedVectorData { groups }, preprocessor))
}

fn waypoint_to_rawdata(point: &Waypoint) -> Result<RawData> {
    let timestamp_ms = match &point.time {
        Some(time) => {
            let value = time.format()?;
            Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(&value)?).timestamp_millis())
        }
        None => None,
    };
    Ok(RawData {
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
    let version = attribute(event, "version")?;
    let source_journey_id = attribute(event, "sourceJourneyId")?.or(attribute(event, "sourceId")?);
    let date = attribute(event, "date")?;
    let journey_date = date
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok());
    if version.as_deref() != Some("1") {
        warn!("Unsupported MemoLanes GPX journey version {version:?}; using generic GPX import");
        return Ok(None);
    }
    if source_journey_id.as_deref().is_none_or(str::is_empty) {
        warn!("MemoLanes GPX journey has no sourceJourneyId; using generic GPX import");
        return Ok(None);
    }
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
        source_journey_id,
        source_revision: attribute(event, "sourceRevision")?,
        journey_date,
        start: optional_time("start")?,
        end: optional_time("end")?,
        segments: Vec::new(),
    }))
}

fn track_point(event: &BytesStart<'_>) -> Result<RawData> {
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
    Ok(RawData {
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
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut prefixes = Vec::new();
    let mut groups = Vec::new();
    let mut current: Option<ImportedJourney> = None;
    let mut segment: Option<Vec<RawData>> = None;
    let mut marked = false;
    let mut invalid_marker = false;

    loop {
        let event = reader.read_event().context("Invalid MemoLanes GPX XML")?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.name().as_ref().to_owned();
                if path.is_empty() && local_name(&name) == "gpx" {
                    for attr in element.attributes() {
                        let attr = attr.context("Invalid GPX namespace declaration")?;
                        if attr.key.as_ref().starts_with("xmlns:")
                            && attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                                == MEMOLANES_NAMESPACE
                        {
                            prefixes.push(attr.key.as_ref()[6..].to_owned());
                        }
                    }
                }

                match local_name(&name) {
                    "trk" if path.len() == 1 => {
                        current = Some(ImportedJourney {
                            source_journey_id: None,
                            source_revision: None,
                            journey_date: None,
                            start: None,
                            end: None,
                            segments: Vec::new(),
                        });
                    }
                    "journey"
                        if path.len() == 3
                            && local_name(&path[1]) == "trk"
                            && local_name(&path[2]) == "extensions"
                            && name.split_once(':').is_some_and(|(prefix, _)| {
                                prefixes.iter().any(|known| known == prefix)
                            }) =>
                    {
                        marked = true;
                        match metadata(&element) {
                            Ok(Some(parsed)) => {
                                if let Some(group) = current.as_mut() {
                                    group.source_journey_id = parsed.source_journey_id;
                                    group.source_revision = parsed.source_revision;
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
                    "trkseg" if path.len() == 2 && local_name(&path[1]) == "trk" => {
                        segment = Some(Vec::new());
                    }
                    "trkpt" if segment.is_some() => {
                        segment.as_mut().unwrap().push(track_point(&element)?);
                    }
                    _ => {}
                }
                if empty {
                    if local_name(&name) == "trkseg" {
                        if let (Some(group), Some(points)) = (current.as_mut(), segment.take()) {
                            group.segments.push(points);
                        }
                    }
                } else {
                    path.push(name);
                }
            }
            Event::End(element) => {
                match local_name(element.name().as_ref()) {
                    "trkseg" => {
                        if let (Some(group), Some(points)) = (current.as_mut(), segment.take()) {
                            group.segments.push(points);
                        }
                    }
                    "trk" => {
                        if let Some(group) = current.take() {
                            groups.push(group);
                        }
                    }
                    _ => {}
                }
                path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if invalid_marker || !marked {
        if !marked {
            warn!("MemoLanes GPX namespace found without valid journey metadata; using generic GPX import");
        }
        return Ok(None);
    }
    if groups.iter().any(|group| group.source_journey_id.is_none()) {
        warn!("MemoLanes GPX contains unmarked tracks; using generic GPX import");
        return Ok(None);
    }
    Ok(Some(ParsedVectorData { groups }))
}

pub fn load_gpx_raw_data(gpx_data: &gpx::Gpx) -> Result<Vec<Vec<RawData>>> {
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
