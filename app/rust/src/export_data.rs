use crate::journey_bitmap::{JourneyBitmap, TileKey, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH};
use crate::journey_vector::JourneyVector;
use crate::storage::RawCsvRow;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use csv::Reader;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Metadata, Track, TrackSegment, Waypoint};
use kml::{Kml, KmlDocument, KmlWriter};
use md5::{Digest, Md5};
use std::{
    collections::{BTreeMap, HashMap},
    io::{Seek, Write},
};
use time::Duration;
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

#[auto_context]
pub fn journey_vector_to_kml_file<T: Write + Seek>(
    journey_vector: &JourneyVector,
    writer: &mut T,
) -> Result<()> {
    let style = kml::types::Style {
        ..kml::types::Style::default()
    };

    let mut elements = vec![Kml::Style(style)];

    for track_segment in &journey_vector.track_segments {
        let mut coords = Vec::new();
        let mut gx_coords = Vec::new();
        track_segment.track_points.iter().for_each(|point| {
            coords.push(kml::types::Coord {
                x: point.longitude,
                y: point.latitude,
                z: None,
            });
            gx_coords.push(kml::types::Element {
                name: "gx:coord".to_owned(),
                content: Some(format!("{} {} {}", point.longitude, point.latitude, 0)),
                ..kml::types::Element::default()
            })
        });
        let geometry = kml::types::LineString {
            coords,
            tessellate: true,
            ..kml::types::LineString::default()
        };

        let placemark = kml::types::Placemark {
            name: Some("export".to_string()),
            children: vec![kml::types::Element {
                name: "gx:Track".to_owned(),
                content: None,
                children: gx_coords,
                ..kml::types::Element::default()
            }],
            geometry: Some(kml::types::Geometry::LineString(geometry)),
            ..kml::types::Placemark::default()
        };
        elements.push(kml::Kml::Placemark(placemark))
    }

    write_kml_document(
        "memolanes".to_owned(),
        "Generated by memolanes".to_owned(),
        elements,
        writer,
    )?;
    Ok(())
}

#[auto_context]
fn write_kml_document<T: Write + Seek>(
    name: String,
    description: String,
    elements: Vec<Kml>,
    writer: &mut T,
) -> Result<()> {
    let document = KmlDocument::<f64> {
        version: kml::KmlVersion::V22,
        attrs: HashMap::from([
            (
                "xmlns".to_owned(),
                "http://www.opengis.net/kml/2.2".to_owned(),
            ),
            (
                "xmlns:gx".to_owned(),
                "http://www.google.com/kml/ext/2.2".to_owned(),
            ),
            (
                "xmlns:kml".to_owned(),
                "http://www.opengis.net/kml/2.2".to_owned(),
            ),
            (
                "xmlns:atom".to_owned(),
                "http://www.w3.org/2005/Atom".to_owned(),
            ),
        ]),
        elements: vec![Kml::Folder(kml::types::Folder {
            name: Some(name),
            description: Some(description),
            elements,
            ..kml::types::Folder::default()
        })],
    };

    let mut writer = KmlWriter::<_, f64>::from_writer(writer);
    let kml = Kml::KmlDocument(document);
    writer.write(&kml)?;
    Ok(())
}

const FOW_FILENAME_MASK1: &str = "olhwjsktri";
const FOW_FILENAME_MASK2: &str = "eizxdwknmo";
const FOW_TILE_HEADER_SIZE: usize = (TILE_WIDTH * TILE_WIDTH * 2) as usize;
const FOW_BLOCK_EXTRA_DATA_SIZE: usize = 3;
const FOW_BLOCK_SIZE: usize = BITMAP_SIZE + FOW_BLOCK_EXTRA_DATA_SIZE;
const FOW_SNAPSHOT_TILE_Z: i32 = 9;
const FOW_SNAPSHOT_TILE_BITSET_SIZE: usize = 2048;
const FOW_SNAPSHOT_METADATA_SIZE: usize = 4012;
const FOW_EARTH_RADIUS_METERS: f64 = 6378137.0;

#[derive(Clone, Copy)]
enum FoWSnapshotFileType {
    Bitmap,
    Hash,
    Layer,
}

fn fow_tile_filename(tile_key: &TileKey) -> String {
    let id = tile_key.y as u32 * MAP_WIDTH as u32 + tile_key.x as u32;
    let digits: Vec<usize> = id
        .to_string()
        .bytes()
        .map(|b| (b - b'0') as usize)
        .collect();
    let id_part = digits
        .iter()
        .map(|&d| FOW_FILENAME_MASK1.as_bytes()[d] as char)
        .collect::<String>();
    let checksum_part = digits
        .iter()
        .map(|&d| FOW_FILENAME_MASK2.as_bytes()[d] as char)
        .collect::<String>();
    let suffix_start = checksum_part.len().saturating_sub(2);
    let name_prefix = format!("{:x}", Md5::digest(id.to_string()))[..4].to_string();
    format!("{name_prefix}{id_part}{}", &checksum_part[suffix_start..])
}

fn fow_snapshot_filename(x: u16, y: u16, z: i32, file_type: FoWSnapshotFileType) -> String {
    const WIDTH_BY_Z: [u32; 14] = [
        1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 4096,
    ];

    let filename_z = z.max(0) as usize;
    let type_offset = match file_type {
        FoWSnapshotFileType::Hash => 74,
        FoWSnapshotFileType::Bitmap | FoWSnapshotFileType::Layer => 0,
    };
    let id = WIDTH_BY_Z[filename_z] * y as u32 + x as u32;
    let checksum_input = id as i32 + FOW_SNAPSHOT_TILE_Z - z + type_offset;
    let id_part = id
        .to_string()
        .bytes()
        .map(|b| FOW_FILENAME_MASK1.as_bytes()[(b - b'0') as usize] as char)
        .collect::<String>();
    let checksum = checksum_input.rem_euclid(100) as usize;
    let suffix = [
        FOW_FILENAME_MASK2.as_bytes()[checksum / 10] as char,
        FOW_FILENAME_MASK2.as_bytes()[checksum % 10] as char,
    ]
    .iter()
    .collect::<String>();
    let name_prefix = format!("{:x}", Md5::digest(checksum_input.to_string()))[..4].to_string();
    format!("{name_prefix}{id_part}{suffix}")
}

fn fow_block_extra_data(bitmap: &[u8; BITMAP_SIZE]) -> [u8; FOW_BLOCK_EXTRA_DATA_SIZE] {
    let visited_count = bitmap.iter().map(|x| x.count_ones()).sum::<u32>();
    debug_assert!(visited_count <= 4096);
    let score = (visited_count * 2 + 1) as u16;
    [0, (score >> 8) as u8, (score & 0xff) as u8]
}

fn serialize_fow_tile(tile: &crate::journey_bitmap::Tile, include_bitmap: bool) -> Result<Vec<u8>> {
    let block_payload_size = if include_bitmap {
        FOW_BLOCK_SIZE
    } else {
        FOW_BLOCK_EXTRA_DATA_SIZE
    };
    let block_count = tile.iter().count();
    let mut data = vec![0_u8; FOW_TILE_HEADER_SIZE + block_count * block_payload_size];

    for (active_block_idx, (block_key, block)) in tile.iter().enumerate() {
        let block_idx = active_block_idx + 1;
        let header_idx = block_key.x() as usize + block_key.y() as usize * TILE_WIDTH as usize;
        let header_offset = header_idx * 2;
        data[header_offset] = (block_idx & 0xff) as u8;
        data[header_offset + 1] = (block_idx >> 8) as u8;

        let payload_offset = FOW_TILE_HEADER_SIZE + active_block_idx * block_payload_size;
        if include_bitmap {
            data[payload_offset..payload_offset + BITMAP_SIZE].copy_from_slice(block.raw_data());
            data[payload_offset + BITMAP_SIZE..payload_offset + FOW_BLOCK_SIZE]
                .copy_from_slice(&fow_block_extra_data(block.raw_data()));
        } else {
            data[payload_offset..payload_offset + FOW_BLOCK_EXTRA_DATA_SIZE]
                .copy_from_slice(&fow_block_extra_data(block.raw_data()));
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&data)?;
    Ok(encoder.finish()?)
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct FoWSnapshotCoord {
    x: u16,
    y: u16,
    z: i32,
}

#[derive(Clone)]
struct FoWSnapshotTile {
    coord: FoWSnapshotCoord,
    blocks: Vec<Option<[u8; BITMAP_SIZE]>>,
}

struct FoWSnapshotExportTile {
    bitmap_filename: String,
    hash_filename: String,
    tile: FoWSnapshotTile,
}

struct FoWSnapshotExportLayer {
    filename: String,
    tile: FoWSnapshotTile,
}

impl FoWSnapshotTile {
    fn from_journey_tile(tile_key: &TileKey, tile: &crate::journey_bitmap::Tile) -> Self {
        let mut snapshot_tile = Self {
            coord: FoWSnapshotCoord {
                x: tile_key.x,
                y: tile_key.y,
                z: FOW_SNAPSHOT_TILE_Z,
            },
            blocks: vec![None; (TILE_WIDTH * TILE_WIDTH) as usize],
        };

        for (block_key, block) in tile.iter() {
            let block_idx = fow_block_index(block_key.x(), block_key.y());
            snapshot_tile.blocks[block_idx] = Some(*block.raw_data());
        }
        snapshot_tile
    }

    fn is_empty(&self) -> bool {
        self.blocks.iter().all(Option::is_none)
    }

    fn count_pixels(&self) -> u64 {
        self.blocks
            .iter()
            .filter_map(|block| block.as_ref())
            .map(|block| {
                block
                    .iter()
                    .map(|byte| byte.count_ones() as u64)
                    .sum::<u64>()
            })
            .sum()
    }

    fn parent_coord(&self) -> FoWSnapshotCoord {
        FoWSnapshotCoord {
            x: self.coord.x >> 1,
            y: self.coord.y >> 1,
            z: self.coord.z - 1,
        }
    }

    fn position_in_parent(&self) -> FoWSnapshotQuadrant {
        match (self.coord.x & 1 != 0, self.coord.y & 1 != 0) {
            (false, false) => FoWSnapshotQuadrant::TopLeft,
            (true, false) => FoWSnapshotQuadrant::TopRight,
            (false, true) => FoWSnapshotQuadrant::BottomLeft,
            (true, true) => FoWSnapshotQuadrant::BottomRight,
        }
    }

    fn empty(coord: FoWSnapshotCoord) -> Self {
        Self {
            coord,
            blocks: vec![None; (TILE_WIDTH * TILE_WIDTH) as usize],
        }
    }

    fn merge_subtile(&mut self, child: &FoWSnapshotTile) {
        let child_position = child.position_in_parent();
        let block_y_offset = if child_position.is_bottom() { 64 } else { 0 };
        let block_x_offset = if child_position.is_right() { 64 } else { 0 };

        for (source_idx, source_block) in child.blocks.iter().enumerate() {
            let Some(source_block) = source_block else {
                continue;
            };
            let source_x = source_idx % TILE_WIDTH as usize;
            let source_y = source_idx / TILE_WIDTH as usize;
            let block_quadrant = FoWSnapshotQuadrant::from_xy(source_x, source_y);
            let dest_x = source_x / 2 + block_x_offset;
            let dest_y = source_y / 2 + block_y_offset;
            let dest_idx = fow_block_index(dest_x as u8, dest_y as u8);
            let dest_block = self.blocks[dest_idx].get_or_insert([0; BITMAP_SIZE]);
            fow_part_merge_block(dest_block, source_block, block_quadrant);
        }
    }
}

#[derive(Clone, Copy)]
enum FoWSnapshotQuadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl FoWSnapshotQuadrant {
    fn from_xy(x: usize, y: usize) -> Self {
        match (x % 2 != 0, y % 2 != 0) {
            (false, false) => Self::TopLeft,
            (true, false) => Self::TopRight,
            (false, true) => Self::BottomLeft,
            (true, true) => Self::BottomRight,
        }
    }

    fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }

    fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }
}

fn fow_block_index(x: u8, y: u8) -> usize {
    x as usize + y as usize * TILE_WIDTH as usize
}

fn fow_downsample_byte_to_nibble(byte: u8) -> u8 {
    let mut result = 0;
    for pair in 0..4 {
        let mask = 0b1100_0000_u8 >> (pair * 2);
        if byte & mask != 0 {
            result |= 1 << (3 - pair);
        }
    }
    result
}

fn fow_part_merge_block(
    dest_block: &mut [u8; BITMAP_SIZE],
    source_block: &[u8; BITMAP_SIZE],
    quadrant: FoWSnapshotQuadrant,
) {
    let row_offset = if quadrant.is_bottom() { 32 } else { 0 };
    let byte_offset = if quadrant.is_right() { 4 } else { 0 };

    for (source_offset, &source_byte) in source_block.iter().enumerate() {
        if source_byte == 0 {
            continue;
        }
        let source_byte_x = source_offset % 8;
        let source_y = source_offset / 8;
        let dest_offset = byte_offset + source_byte_x / 2 + 8 * (row_offset + source_y / 2);
        let nibble = fow_downsample_byte_to_nibble(source_byte);
        if source_byte_x % 2 == 0 {
            dest_block[dest_offset] |= nibble << 4;
        } else {
            dest_block[dest_offset] |= nibble;
        }
    }
}

fn serialize_fow_snapshot_blocks<F>(
    blocks: &[Option<[u8; BITMAP_SIZE]>],
    block_payload_size: usize,
    mut write_payload: F,
) -> Result<Vec<u8>>
where
    F: FnMut(&[u8; BITMAP_SIZE], &mut Vec<u8>, usize),
{
    let block_count = blocks.iter().filter(|block| block.is_some()).count();
    let mut data = vec![0_u8; FOW_TILE_HEADER_SIZE + block_count * block_payload_size];
    let mut active_block_idx = 1;

    for (block_idx, block) in blocks.iter().enumerate() {
        let Some(block) = block else {
            continue;
        };
        let header_offset = block_idx * 2;
        data[header_offset] = (active_block_idx & 0xff) as u8;
        data[header_offset + 1] = (active_block_idx >> 8) as u8;

        let payload_offset = FOW_TILE_HEADER_SIZE + (active_block_idx - 1) * block_payload_size;
        write_payload(block, &mut data, payload_offset);
        active_block_idx += 1;
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&data)?;
    Ok(encoder.finish()?)
}

fn serialize_fow_snapshot_bitmap_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(
        &tile.blocks,
        FOW_BLOCK_SIZE,
        |block, data, payload_offset| {
            data[payload_offset..payload_offset + BITMAP_SIZE].copy_from_slice(block);
            data[payload_offset + BITMAP_SIZE..payload_offset + FOW_BLOCK_SIZE]
                .copy_from_slice(&fow_block_extra_data(block));
        },
    )
}

fn serialize_fow_snapshot_hash_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(
        &tile.blocks,
        FOW_BLOCK_EXTRA_DATA_SIZE,
        |block, data, payload_offset| {
            data[payload_offset..payload_offset + FOW_BLOCK_EXTRA_DATA_SIZE]
                .copy_from_slice(&fow_block_extra_data(block));
        },
    )
}

fn serialize_fow_snapshot_layer_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(&tile.blocks, BITMAP_SIZE, |block, data, payload_offset| {
        data[payload_offset..payload_offset + BITMAP_SIZE].copy_from_slice(block);
    })
}

fn fow_tile_row_area_square_meters(y: u16) -> f64 {
    let y = y.min((MAP_WIDTH as u16 - 1).saturating_sub(y)) as f64;
    let map_width = MAP_WIDTH as f64;
    let lat = |tile_y: f64| {
        (std::f64::consts::PI * (1.0 - 2.0 * tile_y / map_width))
            .sinh()
            .atan()
    };
    let north = lat(y);
    let south = lat(y + 1.0);
    FOW_EARTH_RADIUS_METERS
        * FOW_EARTH_RADIUS_METERS
        * (2.0 * std::f64::consts::PI / map_width)
        * (north.sin() - south.sin()).abs()
}

fn fow_snapshot_metadata(total_area_square_meters: u64) -> Result<Vec<u8>> {
    let mut data = vec![0_u8; FOW_SNAPSHOT_METADATA_SIZE];
    let mut shift_count = 0_u16;
    let mut area = (total_area_square_meters as u128) * 10_000;
    while area < (1_u128 << 44) && shift_count < 44 {
        area <<= 1;
        shift_count += 1;
    }

    let area = area as u64;
    data[5..13].copy_from_slice(&area.to_le_bytes());
    let existing = u16::from_le_bytes([data[10], data[11]]);
    let metadata = 17056_u16.saturating_sub(shift_count << 4);
    let encoded = existing.wrapping_add(metadata);
    data[10..12].copy_from_slice(&encoded.to_le_bytes());
    data[0] = 2;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&data)?;
    Ok(encoder.finish()?)
}

fn serialize_fow_snapshot_tile_index(
    tile_index: &[u8; FOW_SNAPSHOT_TILE_BITSET_SIZE],
) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(tile_index)?;
    Ok(encoder.finish()?)
}

#[auto_context]
pub fn fow_bitmap_to_sync_zip<T: Write + Seek>(
    journey_bitmap: &JourneyBitmap,
    writer: &mut T,
) -> Result<()> {
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::DEFAULT
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o644);

    let mut tiles = journey_bitmap.iter_tiles().collect::<Vec<_>>();
    tiles.sort_by_key(|(tile_key, _)| (**tile_key).clone());
    for (tile_key, tile) in tiles {
        if tile.is_empty() {
            continue;
        }
        zip.start_file(fow_tile_filename(tile_key), options)?;
        zip.write_all(&serialize_fow_tile(tile, true)?)?;
    }
    zip.finish()?;
    Ok(())
}

#[auto_context]
pub fn fow_bitmap_to_snapshot_file<T: Write + Seek>(
    journey_bitmap: &JourneyBitmap,
    writer: &mut T,
) -> Result<()> {
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::DEFAULT
        .compression_method(zip::CompressionMethod::Deflated)
        .system(zip::System::Dos);

    let mut tiles = journey_bitmap.iter_tiles().collect::<Vec<_>>();
    tiles.sort_by_key(|(tile_key, _)| (**tile_key).clone());
    let mut pending_layers: BTreeMap<(i32, u16, u16), FoWSnapshotTile> = BTreeMap::new();
    let mut export_tiles = Vec::new();
    let mut export_layers = Vec::new();
    let mut tile_index = [0_u8; FOW_SNAPSHOT_TILE_BITSET_SIZE];
    let mut total_area_square_meters = 0_u64;

    for (tile_key, tile) in tiles {
        if tile.is_empty() {
            continue;
        }
        let snapshot_tile = FoWSnapshotTile::from_journey_tile(tile_key, tile);
        let bitmap_filename = fow_snapshot_filename(
            snapshot_tile.coord.x,
            snapshot_tile.coord.y,
            snapshot_tile.coord.z,
            FoWSnapshotFileType::Bitmap,
        );
        let hash_filename = fow_snapshot_filename(
            snapshot_tile.coord.x,
            snapshot_tile.coord.y,
            snapshot_tile.coord.z,
            FoWSnapshotFileType::Hash,
        );

        let tile_index_offset =
            ((tile_key.y as usize * MAP_WIDTH as usize) + tile_key.x as usize) / 8;
        if tile_index_offset < tile_index.len() {
            tile_index[tile_index_offset] ^= 128 >> (tile_key.x % 8);
        }

        let tile_area = fow_tile_row_area_square_meters(tile_key.y);
        total_area_square_meters +=
            ((tile_area * snapshot_tile.count_pixels() as f64) / 67_108_864.0) as u64;

        pending_layers.insert(
            (
                snapshot_tile.coord.z,
                snapshot_tile.coord.y,
                snapshot_tile.coord.x,
            ),
            snapshot_tile.clone(),
        );
        export_tiles.push(FoWSnapshotExportTile {
            bitmap_filename,
            hash_filename,
            tile: snapshot_tile,
        });
    }

    while let Some((&key, _)) = pending_layers.iter().next_back() {
        let Some(tile) = pending_layers.remove(&key) else {
            break;
        };

        if tile.coord.z <= 8 && tile.coord.z >= -6 && !tile.is_empty() {
            let filename = fow_snapshot_filename(
                tile.coord.x,
                tile.coord.y,
                tile.coord.z,
                FoWSnapshotFileType::Layer,
            );
            export_layers.push(FoWSnapshotExportLayer {
                filename,
                tile: tile.clone(),
            });
        }

        if tile.coord.z <= -6 {
            break;
        }

        let parent_coord = tile.parent_coord();
        let parent_key = (parent_coord.z, parent_coord.y, parent_coord.x);
        pending_layers
            .entry(parent_key)
            .or_insert_with(|| FoWSnapshotTile::empty(parent_coord))
            .merge_subtile(&tile);
    }

    for export_tile in &export_tiles {
        zip.start_file(format!("Model/*/{}", export_tile.bitmap_filename), options)?;
        zip.write_all(&serialize_fow_snapshot_bitmap_tile(&export_tile.tile)?)?;
    }

    for export_tile in &export_tiles {
        zip.start_file(format!("Model/#/{}", export_tile.hash_filename), options)?;
        zip.write_all(&serialize_fow_snapshot_hash_tile(&export_tile.tile)?)?;
    }

    zip.start_file("Model/#/01abfc750a", options)?;
    zip.write_all(&fow_snapshot_metadata(total_area_square_meters)?)?;
    zip.start_file("Model/#/3389dae361", options)?;
    zip.write_all(&serialize_fow_snapshot_tile_index(&tile_index)?)?;

    for export_layer in &export_layers {
        zip.start_file(format!("Model/~/{}", export_layer.filename), options)?;
        zip.write_all(&serialize_fow_snapshot_layer_tile(&export_layer.tile)?)?;
    }

    zip.finish()?;
    Ok(())
}
