use crate::api::import::ImportPreprocessor;
use crate::gps_processor::{self, Point, RawData};
use crate::import_data::{ImportedJourney, ParsedVectorData};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, NaiveDate, Utc};
use kml::types::{Element, Geometry};
use kml::Kml::Placemark;
use kml::{Kml, KmlReader};
use log::warn;
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::cell::RefCell;
use std::fs;
use std::io::Cursor;

/// Parses KML while retaining MemoLanes Folder and Placemark boundaries.
/// Ordinary KML descriptions are sanitized before generic parsing.
#[auto_context]
pub fn load_kml(file_path: &str) -> Result<(ParsedVectorData, ImportPreprocessor)> {
    let xml = fs::read_to_string(file_path)?;
    if xml.contains("memolanes:") {
        if let Some(parsed) = parse_memolanes_kml(&xml)? {
            return Ok((parsed, ImportPreprocessor::None));
        }
    }
    let (cleaned_xml, _) = read_kml_description_and_remove(&xml)?;
    let mut kml_reader = KmlReader::<_, f64>::from_reader(Cursor::new(cleaned_xml));
    let kml_data = kml_reader.read()?;
    let elements = flatten_kml(kml_data);
    let mut segments = read_track(&elements)?;
    if segments.is_empty() {
        segments = read_line_string(&elements)?;
    }
    let groups = if segments.is_empty() {
        Vec::new()
    } else {
        vec![ImportedJourney::generic(segments)]
    };
    Ok((ParsedVectorData { groups }, ImportPreprocessor::Generic))
}

struct KmlFolder {
    order: usize,
    journey: ImportedJourney,
    version: Option<String>,
    marked: bool,
    placemarks: usize,
    coordinates: Vec<String>,
}

fn parse_memolanes_kml(xml: &str) -> Result<Option<ParsedVectorData>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut folders = Vec::<KmlFolder>::new();
    let mut finished = Vec::<KmlFolder>::new();
    let mut data_name: Option<String> = None;
    let mut in_extended_data = false;
    let mut in_timestamp = false;
    let mut in_placemark = false;
    let mut in_linestring = false;
    let mut line_has_coordinates = false;
    let mut has_marker = false;
    let mut invalid_marker = false;
    let mut unmarked_geometry = false;
    let mut next_order = 0;

    loop {
        match reader.read_event()? {
            Event::Start(event) if local_name(event.name().as_ref()) == "Folder" => {
                folders.push(KmlFolder {
                    order: next_order,
                    journey: ImportedJourney::generic(Vec::new()),
                    version: None,
                    marked: false,
                    placemarks: 0,
                    coordinates: Vec::new(),
                });
                next_order += 1;
            }
            Event::End(event) if local_name(event.name().as_ref()) == "Folder" => {
                if let Some(folder) = folders.pop() {
                    if folder.marked {
                        has_marker = true;
                        if folder.version.as_deref() != Some("1")
                            || folder.journey.source_journey_id.is_none()
                            || folder.journey.journey_date.is_none()
                            || folder.placemarks != folder.coordinates.len()
                        {
                            invalid_marker = true;
                        }
                    } else if folder.placemarks > 0 {
                        unmarked_geometry = true;
                    }
                    finished.push(folder);
                }
            }
            Event::Start(event) if local_name(event.name().as_ref()) == "Placemark" => {
                in_placemark = true;
                if let Some(folder) = folders.last_mut() {
                    folder.placemarks += 1;
                } else {
                    unmarked_geometry = true;
                }
            }
            Event::End(event) if local_name(event.name().as_ref()) == "Placemark" => {
                in_placemark = false;
            }
            Event::Start(event) if local_name(event.name().as_ref()) == "LineString" => {
                in_linestring = true;
                line_has_coordinates = false;
            }
            Event::End(event) if local_name(event.name().as_ref()) == "LineString" => {
                if in_placemark && !line_has_coordinates {
                    if let Some(folder) = folders.last_mut() {
                        folder.coordinates.push(String::new());
                    }
                }
                in_linestring = false;
            }
            Event::Start(event) if local_name(event.name().as_ref()) == "TimeStamp" => {
                in_timestamp = true;
            }
            Event::End(event) if local_name(event.name().as_ref()) == "TimeStamp" => {
                in_timestamp = false;
            }
            Event::Start(event) if local_name(event.name().as_ref()) == "ExtendedData" => {
                in_extended_data = true;
            }
            Event::End(event) if local_name(event.name().as_ref()) == "ExtendedData" => {
                in_extended_data = false;
            }
            Event::Start(event)
                if local_name(event.name().as_ref()) == "Data"
                    && in_extended_data
                    && !in_placemark =>
            {
                data_name = event
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attr| attr.key.as_ref() == "name")
                    .and_then(|attr| {
                        attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .ok()
                            .map(|value| value.into_owned())
                    });
                if data_name
                    .as_deref()
                    .is_some_and(|name| name.starts_with("memolanes:"))
                {
                    if let Some(folder) = folders.last_mut() {
                        folder.marked = true;
                    }
                }
            }
            Event::End(event) if local_name(event.name().as_ref()) == "Data" => {
                data_name = None;
            }
            Event::Start(event)
                if local_name(event.name().as_ref()) == "value" && data_name.is_some() =>
            {
                let raw = reader.read_text(event.name())?;
                let value = quick_xml::escape::unescape(raw.as_ref())?.into_owned();
                if let (Some(folder), Some(name)) = (folders.last_mut(), data_name.as_deref()) {
                    match name {
                        "memolanes:version" => folder.version = Some(value),
                        "memolanes:sourceJourneyId" => {
                            folder.journey.source_journey_id = Some(value)
                        }
                        "memolanes:sourceRevision" => folder.journey.source_revision = Some(value),
                        "memolanes:date" => {
                            folder.journey.journey_date =
                                NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok();
                        }
                        "memolanes:start" => {
                            folder.journey.start = parse_optional_time(&value, "start");
                        }
                        "memolanes:end" => {
                            folder.journey.end = parse_optional_time(&value, "end");
                        }
                        _ => {}
                    }
                }
            }
            Event::Start(event)
                if local_name(event.name().as_ref()) == "when" && in_timestamp && !in_placemark =>
            {
                let raw = reader.read_text(event.name())?;
                let value = quick_xml::escape::unescape(raw.as_ref())?;
                if let Some(folder) = folders.last_mut() {
                    if folder.journey.journey_date.is_none() {
                        folder.journey.journey_date =
                            NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok();
                    }
                }
            }
            Event::Start(event)
                if local_name(event.name().as_ref()) == "coordinates"
                    && in_placemark
                    && in_linestring =>
            {
                let raw = reader.read_text(event.name())?;
                let value = quick_xml::escape::unescape(raw.as_ref())?;
                if let Some(folder) = folders.last_mut() {
                    folder.coordinates.push(value.into_owned());
                    line_has_coordinates = true;
                } else {
                    unmarked_geometry = true;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if invalid_marker || (has_marker && unmarked_geometry) {
        warn!("Incomplete or mixed MemoLanes KML metadata; using ordinary KML import");
        return Ok(None);
    }
    if !has_marker {
        return Ok(None);
    }
    finished.sort_by_key(|folder| folder.order);
    let mut groups = Vec::new();
    for mut folder in finished {
        for coordinates in folder.coordinates {
            let segment = parse_coordinates(&coordinates)?;
            folder.journey.segments.push(segment);
        }
        if !folder.journey.segments.is_empty() {
            groups.push(folder.journey);
        }
    }
    Ok(Some(ParsedVectorData { groups }))
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn parse_optional_time(value: &str, field: &str) -> Option<DateTime<Utc>> {
    match DateTime::parse_from_rfc3339(value) {
        Ok(value) => Some(value.with_timezone(&Utc)),
        Err(error) => {
            warn!("Ignoring invalid MemoLanes KML {field} time {value:?}: {error}");
            None
        }
    }
}

fn parse_coordinates(value: &str) -> Result<Vec<RawData>> {
    value
        .split_whitespace()
        .map(|coordinate| {
            let mut parts = coordinate.split(',');
            let longitude = parts
                .next()
                .context("Missing KML longitude")?
                .parse::<f64>()
                .with_context(|| format!("Invalid KML longitude in {coordinate:?}"))?;
            let latitude = parts
                .next()
                .context("Missing KML latitude")?
                .parse::<f64>()
                .with_context(|| format!("Invalid KML latitude in {coordinate:?}"))?;
            anyhow::ensure!(
                longitude.is_finite() && latitude.is_finite(),
                "Non-finite KML coordinate in {coordinate:?}"
            );
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
        })
        .collect()
}

/// 2bulu generated KML contains HTML tags in <description>, which breaks the KML parser.
/// So let's extract the description early and remove it from the original KML before parsing.
#[auto_context]
fn read_kml_description_and_remove(xml: &str) -> Result<(String, Vec<String>)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut descriptions = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == "description" => {
                let text = reader.read_text(e.name())?;
                descriptions.push(text.into_inner().into_owned());
            }
            Ok(Event::Start(e)) => writer.write_event(Event::Start(e.into_owned()))?,
            Ok(Event::Empty(e)) => writer.write_event(Event::Empty(e.into_owned()))?,
            Ok(Event::End(e)) => writer.write_event(Event::End(e.into_owned()))?,
            Ok(Event::Text(e)) => writer.write_event(Event::Text(e.into_owned()))?,
            Ok(Event::CData(e)) => writer.write_event(Event::CData(e.into_owned()))?,
            Ok(Event::Decl(e)) => writer.write_event(Event::Decl(e.into_owned()))?,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => anyhow::bail!("XML parse error: {e:?}"),
        }
        buf.clear();
    }
    let cleaned_xml = String::from_utf8(writer.into_inner())?;
    Ok((cleaned_xml, descriptions))
}

#[auto_context]
fn read_track(flatten_data: &[Kml]) -> Result<Vec<Vec<RawData>>> {
    let parse_line = |coord: &Option<String>, when: &Option<String>| -> Result<Option<RawData>> {
        let coord: Vec<&str> = match coord {
            Some(coord) => coord.split_whitespace().collect(),
            None => return Ok(None),
        };

        let timestamp = match when {
            None => None,
            Some(when) => Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(when)?)),
        };

        Ok(Some(gps_processor::RawData {
            point: Point {
                latitude: coord[1].parse::<f64>()?,
                longitude: coord[0].parse::<f64>()?,
            },
            timestamp_ms: timestamp.map(|x| x.timestamp_millis()),
            accuracy: None,
            altitude: if coord.len() >= 3 {
                Some(coord[2].parse::<f32>()?)
            } else {
                None
            },
            speed: None,
        }))
    };

    let segments = flatten_data
        .iter()
        .filter_map(|k| match k {
            Kml::Placemark(p) => Some(&p.children),
            _ => None,
        })
        .flat_map(|arr| arr.iter().filter(|e| e.name == "Track"));

    let mut raw_vector_data: Vec<Vec<RawData>> = Vec::new();

    for segment in segments {
        let mut when_list = Vec::new();
        let mut coord_list = Vec::new();
        segment.children.iter().for_each(|e| {
            if e.name == "when" {
                when_list.push(&e.content);
            } else if e.name == "coord" {
                coord_list.push(&e.content);
            }
        });

        let missing_timestamp = when_list.is_empty();
        if !missing_timestamp && when_list.len() != coord_list.len() {
            return Err(anyhow!(
                "number of `when` does not match number of `coord`. when = {}, coord = {}",
                when_list.len(),
                coord_list.len()
            ));
        }

        let mut raw_vector_data_segment: Vec<RawData> = Vec::new();
        for i in 0..coord_list.len() {
            let parse_result = parse_line(
                coord_list[i],
                if missing_timestamp {
                    &None
                } else {
                    when_list[i]
                },
            )?;
            match parse_result {
                None => (),
                Some(raw_data) => raw_vector_data_segment.push(raw_data),
            }
        }
        if !raw_vector_data_segment.is_empty() {
            raw_vector_data.push(raw_vector_data_segment);
        }
    }

    Ok(raw_vector_data)
}

fn read_line_string(flatten_data: &[Kml]) -> Result<Vec<Vec<RawData>>> {
    let mut raw_vector_data: Vec<Vec<RawData>> = Vec::new();

    let convert_to_timestamp = |when: Option<String>| -> Option<i64> {
        match when {
            None => None,
            Some(when) => {
                let datetime = DateTime::parse_from_rfc3339(&when).ok()?;
                Some(datetime.timestamp_millis())
            }
        }
    };

    let extract_time_from_children = |timestamp_element: &Element| -> Option<String> {
        timestamp_element
            .children
            .iter()
            .find(|e| e.name == "when")
            .and_then(|when_element| when_element.content.clone())
    };

    let raw_vector_data_segment: RefCell<Vec<RawData>> = RefCell::new(Vec::new());

    flatten_data.iter().for_each(|k| {
        if let Placemark(p) = k {
            if let Some(geometry) = &p.geometry {
                match geometry {
                    Geometry::Point(point) => {
                        let timestamp_ms = convert_to_timestamp(
                            p.children
                                .iter()
                                .find(|e| e.name == "TimeStamp")
                                .and_then(extract_time_from_children),
                        );
                        raw_vector_data_segment.borrow_mut().push(RawData {
                            point: Point {
                                latitude: point.coord.y,
                                longitude: point.coord.x,
                            },
                            timestamp_ms,
                            accuracy: None,
                            altitude: None,
                            speed: None,
                        });
                    }
                    Geometry::LineString(line_string) => {
                        line_string.coords.iter().for_each(|coord| {
                            raw_vector_data_segment.borrow_mut().push(RawData {
                                point: Point {
                                    latitude: coord.y,
                                    longitude: coord.x,
                                },
                                timestamp_ms: None,
                                accuracy: None,
                                altitude: None,
                                speed: None,
                            });
                        });

                        // we treat different `LineString` as different segments
                        if !raw_vector_data_segment.borrow().is_empty() {
                            raw_vector_data.push(raw_vector_data_segment.replace(Vec::new()));
                        }
                    }
                    _ => (),
                }
            }
        }
    });
    if !raw_vector_data_segment.borrow().is_empty() {
        raw_vector_data.push(raw_vector_data_segment.into_inner());
    }
    Ok(raw_vector_data)
}

fn flatten_kml(kml: Kml) -> Vec<Kml> {
    let flatten_elements =
        |elements: Vec<Kml>| elements.into_iter().flat_map(flatten_kml).collect();
    match kml {
        Kml::KmlDocument(d) => flatten_elements(d.elements),
        Kml::Document { attrs: _, elements } => flatten_elements(elements),
        Kml::Folder(folder) => flatten_elements(folder.elements),
        k => vec![k],
    }
}
