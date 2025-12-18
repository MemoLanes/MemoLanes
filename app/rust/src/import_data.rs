use crate::api::import::{ImportPreprocessor, JourneyInfo};
use crate::flight_track_processor;
use crate::gps_processor::{
    Point, PreprocessedData, ProcessResult, RawData, SegmentGapRules, DEFAULT_SEGMENT_GAP_RULES,
};
use crate::journey_bitmap::{
    self, Block, BlockKey, JourneyBitmap, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH,
};
use crate::journey_date_picker::JourneyDatePicker;
use crate::journey_header::JourneyKind;
use crate::{
    gps_processor::{self, GpsPreprocessor},
    journey_vector::{JourneyVector, TrackPoint},
};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::{DateTime, Local, TimeZone, Utc};
use flate2::read::ZlibDecoder;
use gpx::{read, Waypoint};
use kml::types::{Element, Geometry};
use kml::Kml::Placemark;
use kml::{Kml, KmlReader};
use quick_xml::events::Event;
use quick_xml::{Reader, Writer};
use std::cell::RefCell;
use std::io::Cursor;
use std::result::Result::Ok;
use std::{fs, vec};
use std::{fs::File, io::BufReader, io::Read, path::Path};

struct FoWTileId {
    x: u16,
    y: u16,
}

impl FoWTileId {
    const FILENAME_MASK1: &'static str = "olhwjsktri";

    pub fn from_filename(filename: &str) -> Option<Self> {
        if filename.len() < 6 {
            return None;
        }
        let id_part = &filename[4..(filename.len() - 2)];
        let mut id: u32 = 0;
        for c in id_part.chars() {
            let v = FoWTileId::FILENAME_MASK1.find(c)?;
            id = id * 10 + v as u32;
        }
        if id >= (MAP_WIDTH * MAP_WIDTH) as u32 {
            return None;
        }
        Some(FoWTileId {
            x: (id % MAP_WIDTH as u32) as u16,
            y: (id / MAP_WIDTH as u32) as u16,
        })
    }
}

#[auto_context]
fn parse_fow_bitmap_file<R: Read>(
    file: R,
    filename: &str,
    journey_bitmap: &mut JourneyBitmap,
    warnings: &mut Vec<String>,
) -> Result<()> {
    const TILE_HEADER_LEN: i64 = TILE_WIDTH * TILE_WIDTH;
    const TILE_HEADER_SIZE: usize = (TILE_HEADER_LEN * 2) as usize;
    const BLOCK_BITMAP_SIZE: usize = BITMAP_SIZE;
    const BLOCK_EXTRA_DATA: usize = 3;
    const BLOCK_SIZE: usize = BLOCK_BITMAP_SIZE + BLOCK_EXTRA_DATA;

    match FoWTileId::from_filename(filename) {
        None => warnings.push(format!("unexpected file: {filename}")),
        Some(id) => {
            let mut tile = journey_bitmap::Tile::new();
            let mut data = Vec::new();
            ZlibDecoder::new(file).read_to_end(&mut data)?;

            let header = &data[0..TILE_HEADER_SIZE];
            for i in 0..TILE_HEADER_LEN {
                // parse two u8 as a single u16 according to little endian
                let index = (i as usize) * 2;
                let block_idx: u16 = (header[index] as u16) | ((header[index + 1] as u16) << 8);
                if block_idx > 0 {
                    let block_key =
                        BlockKey::from_x_y((i % TILE_WIDTH) as u8, (i / TILE_WIDTH) as u8);
                    let start_offset = TILE_HEADER_SIZE + ((block_idx - 1) as usize) * BLOCK_SIZE;
                    let end_offset = start_offset + BLOCK_BITMAP_SIZE;
                    let mut bitmap: [u8; BLOCK_BITMAP_SIZE] = [0; BLOCK_BITMAP_SIZE];
                    bitmap.copy_from_slice(&data[start_offset..end_offset]);
                    let block = Block::new_with_data(bitmap);
                    tile.set(block_key, block);
                }
            }
            journey_bitmap.tiles.insert((id.x, id.y), tile);
        }
    }
    Ok(())
}

#[auto_context]
pub fn load_fow_sync_data(mldx_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    let mut warnings: Vec<String> = Vec::new();

    let mut zip = zip::ZipArchive::new(File::open(mldx_file_path)?)?;
    let has_sync_folder = zip
        .file_names()
        .any(|name| name.to_lowercase().contains("sync/"));

    let mut journey_bitmap = JourneyBitmap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let filename = file.name().to_lowercase();
        // the check below are just best effort.
        // if there is a sync folder, skip all other files
        if has_sync_folder && !filename.contains("sync/") {
            continue;
        }
        let filename = Path::file_name(Path::new(&filename))
            .and_then(|x| x.to_str())
            .unwrap_or("");
        if filename.is_empty() || filename.starts_with('.') || file.is_dir() {
            continue;
        }
        parse_fow_bitmap_file(file, filename, &mut journey_bitmap, &mut warnings)?;
    }

    let warnings = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n"))
    };

    if journey_bitmap.tiles.is_empty() {
        Err(anyhow!(
            "empty data. warnings: {}",
            warnings.unwrap_or("".to_owned())
        ))
    } else {
        Ok((journey_bitmap, warnings))
    }
}

#[auto_context]
pub fn load_fow_snapshot_data(fwss_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    let mut warnings: Vec<String> = Vec::new();
    let mut zip = zip::ZipArchive::new(File::open(fwss_file_path)?)?;

    let mut journey_bitmap = JourneyBitmap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let fwss_subfilename = file.name().to_lowercase();

        if !fwss_subfilename.contains("model/*/") {
            continue;
        }

        let filename = Path::file_name(Path::new(&fwss_subfilename))
            .and_then(|x| x.to_str())
            .unwrap_or("");
        if filename.is_empty() || filename.starts_with('.') || file.is_dir() {
            continue;
        }
        parse_fow_bitmap_file(file, filename, &mut journey_bitmap, &mut warnings)?;
    }

    let warnings = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n"))
    };

    if journey_bitmap.tiles.is_empty() {
        Err(anyhow!(
            "empty data. warnings: {}",
            warnings.unwrap_or_default()
        ))
    } else {
        Ok((journey_bitmap, warnings))
    }
}

fn recommend_import_preprocessor_from_gpx(gpx: &gpx::Gpx) -> ImportPreprocessor {
    let creator = gpx.creator.as_deref().unwrap_or("").to_ascii_lowercase();

    if creator.contains("stepofmyworld") || creator.contains("yourapp") {
        ImportPreprocessor::StepOfMyWorld
    } else {
        ImportPreprocessor::Generic
    }
}

#[auto_context]
pub fn load_gpx(file_path: &str) -> Result<(Vec<Vec<RawData>>, ImportPreprocessor)> {
    let gpx = read(BufReader::new(File::open(file_path)?))?;
    let raw_data = load_gpx_raw_data(&gpx)?;
    let preprocessor = recommend_import_preprocessor_from_gpx(&gpx);
    Ok((raw_data, preprocessor))
}

pub fn load_gpx_raw_data(gpx_data: &gpx::Gpx) -> Result<Vec<Vec<RawData>>> {
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

    let waypoint_to_rawdata = |point: &Waypoint| -> Result<RawData> {
        Ok(RawData {
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

    // TODO KML currently has no additional processors.
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
            Ok(Event::Start(e)) if e.name().as_ref() == b"description" => {
                let text = reader.read_text(e.name())?;
                descriptions.push(text.to_string());
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
        Kml::Folder { attrs: _, elements } => flatten_elements(elements),
        k => vec![k],
    }
}

pub fn journey_vector_from_raw_data_with_gps_preprocessor(
    raw_data: &[Vec<RawData>],
    enable_preprocessor: bool,
) -> Option<JourneyVector> {
    journey_vector_from_raw_data_with_rules(
        raw_data,
        enable_preprocessor,
        DEFAULT_SEGMENT_GAP_RULES,
    )
}

pub fn journey_vector_from_raw_data_with_rules(
    raw_data: &[Vec<RawData>],
    enable_preprocessor: bool,
    segment_gap_policy: SegmentGapRules,
) -> Option<JourneyVector> {
    let processed_data = raw_data.iter().flat_map(move |x| {
        // we handle each segment separately
        let mut gps_preprocessor = GpsPreprocessor::new_with_rules(segment_gap_policy);
        let mut first = true;
        x.iter().map(move |raw_data| {
            let process_result = if enable_preprocessor {
                gps_preprocessor.preprocess(raw_data)
            } else if first {
                first = false;
                ProcessResult::NewSegment
            } else {
                ProcessResult::Append
            };

            Ok(PreprocessedData {
                timestamp_sec: raw_data.timestamp_ms.map(|x| x / 1000),
                track_point: TrackPoint {
                    latitude: raw_data.point.latitude,
                    longitude: raw_data.point.longitude,
                },
                process_result,
            })
        })
    });

    gps_processor::build_journey_vector(processed_data, None)
        .expect("Impossible, `preprocessed_data` does not contain error")
}

pub fn journey_vector_from_raw_data_with_flight_track_processor(
    raw_data: &[Vec<RawData>],
) -> Option<JourneyVector> {
    flight_track_processor::process(raw_data)
}

pub fn journey_info_from_raw_vector_data(raw_vector_data: &[Vec<RawData>]) -> JourneyInfo {
    let time_from_raw_data = |raw_data: &RawData| {
        raw_data
            .timestamp_ms
            .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
    };

    let mut journey_date_picker = JourneyDatePicker::new();
    for segment in raw_vector_data {
        for raw_data in segment {
            if let Some(timestamp) = time_from_raw_data(raw_data) {
                journey_date_picker.add_point(
                    timestamp,
                    &TrackPoint {
                        latitude: raw_data.point.latitude,
                        longitude: raw_data.point.longitude,
                    },
                );
            }
        }
    }

    let journey_date = journey_date_picker
        .pick_journey_date()
        .unwrap_or_else(|| Local::now().date_naive());

    JourneyInfo {
        journey_date,
        start_time: journey_date_picker.min_time(),
        end_time: journey_date_picker.max_time(),
        note: None,
        journey_kind: JourneyKind::DefaultKind,
    }
}
