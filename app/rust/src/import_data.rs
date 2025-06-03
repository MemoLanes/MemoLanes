use crate::api::import::JourneyInfo;
use crate::gps_processor::{Point, PreprocessedData, ProcessResult, RawData};
use crate::journey_bitmap::{
    self, Block, BlockKey, JourneyBitmap, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH,
};
use crate::journey_header::JourneyKind;
use crate::{
    gps_processor::{self, GpsPreprocessor},
    journey_vector::{JourneyVector, TrackPoint},
};
use anyhow::Result;
use chrono::{DateTime, Local, TimeZone, Utc};
use flate2::read::ZlibDecoder;
use gpx::{read, Time};
use kml::types::Geometry;
use kml::{Kml, KmlReader};
use std::result::Result::Ok;
use std::vec;
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

pub fn load_fow_sync_data(mldx_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    const TILE_HEADER_LEN: i64 = TILE_WIDTH * TILE_WIDTH;
    const TILE_HEADER_SIZE: usize = (TILE_HEADER_LEN * 2) as usize;
    const BLOCK_BITMAP_SIZE: usize = BITMAP_SIZE;
    const BLOCK_EXTRA_DATA: usize = 3;
    const BLOCK_SIZE: usize = BLOCK_BITMAP_SIZE + BLOCK_EXTRA_DATA;

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
        match FoWTileId::from_filename(filename) {
            None => warnings.push(format!("unexpected file: {}", file.name())),
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
                        let start_offset =
                            TILE_HEADER_SIZE + ((block_idx - 1) as usize) * BLOCK_SIZE;
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

pub fn load_gpx(file_path: &str) -> Result<Vec<Vec<RawData>>> {
    let convert_to_timestamp = |time: &Option<Time>| -> Result<Option<DateTime<Utc>>> {
        let timestamp: Option<DateTime<Utc>> = match &time {
            Some(t) => Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(
                &t.format()?,
            )?)),
            None => None,
        };
        Ok(timestamp)
    };
    let gpx_data = read(BufReader::new(File::open(file_path)?))?;
    let mut raw_vector_data: Vec<Vec<RawData>> = Vec::new();
    for track in gpx_data.tracks {
        for segment in track.segments {
            let mut raw_vector_data_segment: Vec<RawData> = Vec::new();
            for point in segment.points {
                let timestamp = convert_to_timestamp(&point.time)?;
                raw_vector_data_segment.push(gps_processor::RawData {
                    point: Point {
                        latitude: point.point().y(),
                        longitude: point.point().x(),
                    },
                    timestamp_ms: timestamp.map(|x| x.timestamp_millis()),
                    accuracy: point.hdop.map(|hdop| hdop as f32),
                    altitude: point.elevation.map(|value| value as f32),
                    speed: point.speed.map(|value| value as f32),
                });
            }
            if !raw_vector_data_segment.is_empty() {
                raw_vector_data.push(raw_vector_data_segment);
            }
        }
    }

    Ok(raw_vector_data)
}

pub fn load_kml(file_path: &str) -> Result<Vec<Vec<RawData>>> {
    let kml_data =
        KmlReader::<_, f64>::from_reader(BufReader::new(File::open(file_path)?)).read()?;
    let flatten_data = flatten_kml(kml_data);
    let mut raw_vector_data = read_track(&flatten_data)?;
    if raw_vector_data.is_empty() {
        raw_vector_data = read_line_string(&flatten_data)?
    }
    Ok(raw_vector_data)
}

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
    flatten_data.iter().for_each(|k| {
        let mut raw_vector_data_segment: Vec<RawData> = Vec::new();
        if let Kml::Placemark(p) = k {
            if let Some(Geometry::LineString(p)) = &p.geometry {
                p.coords.iter().for_each(|coord| {
                    raw_vector_data_segment.push(RawData {
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
            }
        }
        raw_vector_data.push(raw_vector_data_segment);
    });
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

pub fn journey_vector_from_raw_data(
    raw_data: Vec<Vec<RawData>>,
    run_preprocessor: bool,
) -> Option<JourneyVector> {
    let processed_data = raw_data.into_iter().flat_map(move |x| {
        // we handle each segment separately
        let mut gps_preprocessor = GpsPreprocessor::new();
        let mut first = true;
        x.into_iter().map(move |raw_data| {
            let process_result = if run_preprocessor {
                gps_preprocessor.preprocess(&raw_data)
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

    match gps_processor::build_vector_journey(processed_data)
        .expect("Impossible, `preprocessed_data` does not contain error")
    {
        None => None,
        Some(ongoing_journey) => Some(ongoing_journey.journey_vector),
    }
}

pub fn journey_info_from_raw_vector_data(raw_vector_data: &[Vec<RawData>]) -> JourneyInfo {
    let time_from_raw_data = |raw_data: &RawData| {
        raw_data
            .timestamp_ms
            .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
    };
    let start_time = raw_vector_data
        .first()
        .and_then(|x| x.first())
        .and_then(time_from_raw_data);

    let end_time = raw_vector_data
        .last()
        .and_then(|x| x.last())
        .and_then(time_from_raw_data);

    let local_date_from_time = start_time
        .or(end_time)
        .map(|time| time.with_timezone(&Local).date_naive());

    let journey_date = local_date_from_time.unwrap_or_else(|| Local::now().date_naive());
    JourneyInfo {
        journey_date,
        start_time,
        end_time,
        note: None,
        journey_kind: JourneyKind::DefaultKind,
    }
}
