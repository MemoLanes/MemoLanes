use crate::api::import::ImportPreprocessor;
use crate::gps_processor::{self, Point, RawData};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Utc};
use kml::types::{Element, Geometry};
use kml::Kml::Placemark;
use kml::{Kml, KmlReader};
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::cell::RefCell;
use std::fs;
use std::io::Cursor;

/// Load and parse KML safely, skipping invalid <description> blocks.
#[auto_context]
pub fn load_kml(file_path: &str) -> Result<(Vec<Vec<RawData>>, ImportPreprocessor)> {
    let xml = fs::read_to_string(file_path)?;
    let (cleaned_xml, _descriptions) = read_kml_description_and_remove(&xml)?;
    // TODO: pass _descriptions to journey_info if needed later
    let mut kml_reader = KmlReader::<_, f64>::from_reader(Cursor::new(cleaned_xml));
    let kml_data = kml_reader.read()?;
    let flatten_data = flatten_kml(kml_data);
    let mut raw_vector_data = read_track(&flatten_data)?;
    if raw_vector_data.is_empty() {
        raw_vector_data = read_line_string(&flatten_data)?
    }

    // TODO: we currently do not have preprocessor detection for KML
    Ok((raw_vector_data, ImportPreprocessor::Generic))
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
