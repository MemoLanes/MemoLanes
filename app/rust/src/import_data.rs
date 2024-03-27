use crate::gps_processor::{PreprocessedData, ProcessResult, RawData};
use crate::journey_bitmap::{self, Block, JourneyBitmap, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH};
use crate::{
    gps_processor::{self, GpsProcessor},
    journey_vector::{JourneyVector, TrackPoint},
};
use anyhow::{Error, Ok, Result};
use chrono::{DateTime, Utc};
use flate2::read::ZlibDecoder;
use gpx::read;
use kml::{Kml, KmlReader};
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

pub fn load_fow_sync_data(zip_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    const TILE_HEADER_LEN: i64 = TILE_WIDTH * TILE_WIDTH;
    const TILE_HEADER_SIZE: usize = (TILE_HEADER_LEN * 2) as usize;
    const BLOCK_BITMAP_SIZE: usize = BITMAP_SIZE;
    const BLOCK_EXTRA_DATA: usize = 3;
    const BLOCK_SIZE: usize = BLOCK_BITMAP_SIZE + BLOCK_EXTRA_DATA;

    let mut warnings: Vec<String> = Vec::new();

    let mut zip = zip::ZipArchive::new(File::open(zip_file_path)?)?;
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
                        let block_x = (i % TILE_WIDTH) as u8;
                        let block_y = (i / TILE_WIDTH) as u8;
                        let start_offset =
                            TILE_HEADER_SIZE + ((block_idx - 1) as usize) * BLOCK_SIZE;
                        let end_offset = start_offset + BLOCK_BITMAP_SIZE;
                        let mut bitmap: [u8; BLOCK_BITMAP_SIZE] = [0; BLOCK_BITMAP_SIZE];
                        bitmap.copy_from_slice(&data[start_offset..end_offset]);
                        let block = Block::new_with_data(bitmap);
                        tile.blocks.insert((block_x, block_y), block);
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

// TODO: we need to return the timestamp to outside
fn load_vector_data(
    raw_data_segments: impl Iterator<Item = impl Iterator<Item = Result<RawData>>>,
    run_preprocessor: bool,
) -> Result<JourneyVector> {
    let preprocessed_data = raw_data_segments.flat_map(|raw_data_list| {
        // We need a new processor for each new segment. We want the first one to be `NewSegment`.
        let mut gps_processor = GpsProcessor::new();
        raw_data_list.map(move |x| {
            x.map(|curr_data| {
                let process_result = if run_preprocessor {
                    // This is ugly but fine
                    let mut result = ProcessResult::Append;
                    gps_processor
                        .preprocess(curr_data.clone(), |_last_data, _curr_data, result_| {
                            result = result_
                        });
                    result
                } else {
                    ProcessResult::Append
                };

                PreprocessedData {
                    timestamp_sec: curr_data.timestamp_ms.map(|x| x / 1000),
                    track_point: TrackPoint {
                        latitude: curr_data.latitude,
                        longitude: curr_data.longitude,
                    },
                    process_result,
                }
            })
        })
    });

    let journey_vector = match gps_processor::build_vector_journey(preprocessed_data)? {
        Some(ongoing_journey) => ongoing_journey.journey_vector,
        None => return Err(Error::msg("No data found")),
    };
    Ok(journey_vector)
}

pub fn load_gpx(file_path: &str, run_preprocessor: bool) -> Result<JourneyVector> {
    let gpx_data = read(BufReader::new(File::open(file_path)?))?;
    let raw_data_segments = gpx_data.tracks.iter().flat_map(|track| {
        track.segments.iter().map(|segment| {
            segment.points.iter().map(|point| {
                let timestamp = match &point.time {
                    Some(time) => Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(
                        &time.format()?,
                    )?)),
                    None => None,
                };
                Ok(gps_processor::RawData {
                    latitude: point.point().y(),
                    longitude: point.point().x(),
                    timestamp_ms: timestamp.map(|x| x.timestamp_millis()),
                    accuracy: point.hdop.map(|hdop| hdop as f32),
                    altitude: point.elevation.map(|value| value as f32),
                    speed: point.speed.map(|value| value as f32),
                })
            })
        })
    });
    load_vector_data(raw_data_segments, run_preprocessor)
}

pub fn load_kml(file_path: &str, run_preprocessor: bool) -> Result<JourneyVector> {
    let kml_data =
        KmlReader::<_, f64>::from_reader(BufReader::new(File::open(file_path)?)).read()?;
    let mut whens = Vec::new();
    let mut coords = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    flatten_kml(vec![kml_data])
        .into_iter()
        .filter_map(|k| match k {
            Kml::Placemark(p) => Some(p.children),
            _ => None,
        })
        .flat_map(|arr| arr.into_iter().filter(|e| e.name == "Track"))
        .for_each(|e| {
            e.children.into_iter().for_each(|e| {
                if e.name == "when" {
                    whens.push(e.content);
                } else if e.name == "coord" {
                    coords.push(e.content);
                }
            })
        });
    let mut raw_data_segments = Vec::new();

    for (when, coord) in whens.iter().zip(coords.iter()) {
        let splitted: Vec<&str> = match &coord {
            Some(coord) => coord.split_whitespace().collect(),
            None => {
                continue;
            }
        };
        let timestamp = match &when {
            Some(time) => Some(DateTime::<Utc>::from(DateTime::parse_from_rfc3339(&time)?)),
            None => None,
        };
        raw_data_segments.push(Ok(gps_processor::RawData {
            latitude: splitted[1].parse::<f64>()?,
            longitude: splitted[0].parse::<f64>()?,
            timestamp_ms: timestamp.map(|x| x.timestamp_millis()),
            accuracy: None,
            altitude: Some(splitted[2].parse::<f32>()?),
            speed: None,
        }))
    }
    load_vector_data(raw_data_segments.into_iter(), run_preprocessor)
}

fn flatten_kml(kml: Vec<Kml>) -> Vec<Kml> {
    kml.into_iter()
        .flat_map(|k| match k {
            Kml::KmlDocument(d) => flatten_kml(d.elements),
            Kml::Document { attrs: _, elements } => flatten_kml(elements),
            Kml::Folder { attrs: _, elements } => flatten_kml(elements),
            k => vec![k],
        })
        .collect()
}
