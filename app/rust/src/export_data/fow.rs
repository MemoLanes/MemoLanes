use crate::journey_bitmap::{JourneyBitmap, TileKey, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH};
use crate::journey_vector::JourneyVector;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use md5::{Digest, Md5};
use std::collections::BTreeMap;
use std::io::{Seek, Write};

const FOW_FILENAME_ID_DIGIT_MASK: &str = "olhwjsktri";
const FOW_FILENAME_CHECKSUM_DIGIT_MASK: &str = "eizxdwknmo";
const FOW_FILENAME_HASH_TYPE_OFFSET: i32 = 74;
// Widths used by the FWSS filename id algorithm. These are part of the
// filename encoding, not a general Web Mercator grid description.
const FOW_FILENAME_WIDTH_BY_Z: [u32; 14] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 4096,
];
const FOW_TILE_HEADER_SIZE: usize = (TILE_WIDTH * TILE_WIDTH * 2) as usize;
const FOW_BLOCK_EXTRA_DATA_SIZE: usize = 3;
const FOW_SNAPSHOT_BASE_TILE_Z: i32 = 9;
const FOW_SNAPSHOT_MAX_LAYER_Z: i32 = FOW_SNAPSHOT_BASE_TILE_Z - 1;
const FOW_SNAPSHOT_MIN_LAYER_Z: i32 = -6;
const FOW_SNAPSHOT_TILE_BITSET_SIZE: usize = (MAP_WIDTH * MAP_WIDTH / 8) as usize;
const FOW_SNAPSHOT_METADATA_SIZE: usize = 4012;
const FOW_EARTH_RADIUS_METERS: f64 = 6378137.0;
const FOW_PIXELS_PER_BITMAP_BLOCK: u64 = (BITMAP_SIZE * 8) as u64;
const FOW_PIXELS_PER_BASE_TILE: u64 =
    (TILE_WIDTH * TILE_WIDTH) as u64 * FOW_PIXELS_PER_BITMAP_BLOCK;
const FOW_HASH_BLOCK_PREFIX: u8 = 35;
const FOW_HASH_BLOCK_COUNT_HIGH_OFFSET: u8 = 192;
const FOW_METADATA_AREA_SCALE: u128 = 10_000;
const FOW_METADATA_AREA_NORMALIZE_BITS: u16 = 44;
const FOW_METADATA_BASE_VALUE: u16 = 17056;
const FOW_METADATA_VERSION: u8 = 2;
const FOW_METADATA_AREA_RANGE: std::ops::Range<usize> = 5..13;
const FOW_METADATA_ENCODED_RANGE: std::ops::Range<usize> = 10..12;
// Reserved FWSS filenames: md5("#")[..10] for metadata and md5("*")[..10]
// for the z9 tile index that describes Model/* entries.
const FOW_SNAPSHOT_METADATA_FILENAME: &str = "01abfc750a";
const FOW_SNAPSHOT_TILE_INDEX_FILENAME: &str = "3389dae361";

#[derive(Clone, Copy)]
enum FoWSnapshotFileType {
    Bitmap,
    Hash,
    Layer,
}

fn fow_snapshot_filename(x: u16, y: u16, z: i32, file_type: FoWSnapshotFileType) -> String {
    let filename_z = z.max(0) as usize;
    let type_offset = match file_type {
        FoWSnapshotFileType::Hash => FOW_FILENAME_HASH_TYPE_OFFSET,
        FoWSnapshotFileType::Bitmap | FoWSnapshotFileType::Layer => 0,
    };
    let id = FOW_FILENAME_WIDTH_BY_Z[filename_z] * y as u32 + x as u32;
    let checksum_input = id as i32 + FOW_SNAPSHOT_BASE_TILE_Z - z + type_offset;
    let id_part = id
        .to_string()
        .bytes()
        .map(|b| FOW_FILENAME_ID_DIGIT_MASK.as_bytes()[(b - b'0') as usize] as char)
        .collect::<String>();
    let checksum = checksum_input.rem_euclid(100) as usize;
    let suffix = [
        FOW_FILENAME_CHECKSUM_DIGIT_MASK.as_bytes()[checksum / 10] as char,
        FOW_FILENAME_CHECKSUM_DIGIT_MASK.as_bytes()[checksum % 10] as char,
    ]
    .iter()
    .collect::<String>();
    let name_prefix = hex::encode(Md5::digest(checksum_input.to_string()))[..4].to_string();
    format!("{name_prefix}{id_part}{suffix}")
}

fn fow_bitmap_block_extra_data(bitmap: &[u8; BITMAP_SIZE]) -> [u8; FOW_BLOCK_EXTRA_DATA_SIZE] {
    let visited_count = bitmap.iter().map(|x| x.count_ones()).sum::<u32>();
    debug_assert!(visited_count <= FOW_PIXELS_PER_BITMAP_BLOCK as u32);
    let score = (visited_count * 2 + 1) as u16;
    [0, (score >> 8) as u8, (score & 0xff) as u8]
}

fn fow_hash_block_payload(bitmap: &[u8; BITMAP_SIZE]) -> [u8; FOW_BLOCK_EXTRA_DATA_SIZE] {
    let visited_count = bitmap.iter().map(|x| x.count_ones()).sum::<u32>();
    debug_assert!(visited_count <= FOW_PIXELS_PER_BITMAP_BLOCK as u32);
    let visited_count = visited_count as u16;
    [
        FOW_HASH_BLOCK_PREFIX,
        FOW_HASH_BLOCK_COUNT_HIGH_OFFSET + ((visited_count >> 8) as u8),
        (visited_count & 0xff) as u8,
    ]
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
    blocks: BTreeMap<usize, [u8; BITMAP_SIZE]>,
}

impl FoWSnapshotTile {
    fn from_journey_tile(tile_key: &TileKey, tile: &crate::journey_bitmap::Tile) -> Self {
        let mut snapshot_tile = Self {
            coord: FoWSnapshotCoord {
                x: tile_key.x,
                y: tile_key.y,
                z: FOW_SNAPSHOT_BASE_TILE_Z,
            },
            blocks: BTreeMap::new(),
        };

        for (block_key, block) in tile.iter() {
            let block_idx = fow_block_index(block_key.x(), block_key.y());
            snapshot_tile.blocks.insert(block_idx, *block.raw_data());
        }
        snapshot_tile
    }

    fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    fn count_pixels(&self) -> u64 {
        self.blocks
            .values()
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
            blocks: BTreeMap::new(),
        }
    }

    fn merge_subtile(&mut self, child: &FoWSnapshotTile) {
        let child_position = child.position_in_parent();
        let block_y_offset = if child_position.is_bottom() { 64 } else { 0 };
        let block_x_offset = if child_position.is_right() { 64 } else { 0 };

        for (&source_idx, source_block) in &child.blocks {
            let source_x = source_idx % TILE_WIDTH as usize;
            let source_y = source_idx / TILE_WIDTH as usize;
            let block_quadrant = FoWSnapshotQuadrant::from_xy(source_x, source_y);
            let dest_x = source_x / 2 + block_x_offset;
            let dest_y = source_y / 2 + block_y_offset;
            let dest_idx = fow_block_index(dest_x as u8, dest_y as u8);
            let dest_block = self.blocks.entry(dest_idx).or_insert([0; BITMAP_SIZE]);
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
        match (!x.is_multiple_of(2), !y.is_multiple_of(2)) {
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
    blocks: &BTreeMap<usize, [u8; BITMAP_SIZE]>,
    mut write_payload: F,
) -> Result<Vec<u8>>
where
    F: FnMut(&[u8; BITMAP_SIZE], &mut ZlibEncoder<Vec<u8>>) -> Result<()>,
{
    let mut header = vec![0_u8; FOW_TILE_HEADER_SIZE];

    for (active_block_idx, &block_idx) in (1..).zip(blocks.keys()) {
        let header_offset = block_idx * 2;
        header[header_offset] = (active_block_idx & 0xff) as u8;
        header[header_offset + 1] = (active_block_idx >> 8) as u8;
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&header)?;
    for block in blocks.values() {
        write_payload(block, &mut encoder)?;
    }
    Ok(encoder.finish()?)
}

fn serialize_fow_snapshot_bitmap_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(&tile.blocks, |block, encoder| {
        encoder.write_all(block)?;
        encoder.write_all(&fow_bitmap_block_extra_data(block))?;
        Ok(())
    })
}

fn serialize_fow_snapshot_hash_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(&tile.blocks, |block, encoder| {
        encoder.write_all(&fow_hash_block_payload(block))?;
        Ok(())
    })
}

fn serialize_fow_snapshot_layer_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(&tile.blocks, |block, encoder| {
        encoder.write_all(block)?;
        Ok(())
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
    let mut area = (total_area_square_meters as u128) * FOW_METADATA_AREA_SCALE;
    while area < (1_u128 << FOW_METADATA_AREA_NORMALIZE_BITS)
        && shift_count < FOW_METADATA_AREA_NORMALIZE_BITS
    {
        area <<= 1;
        shift_count += 1;
    }

    let area = area as u64;
    data[FOW_METADATA_AREA_RANGE].copy_from_slice(&area.to_le_bytes());
    let existing = u16::from_le_bytes([data[10], data[11]]);
    let metadata = FOW_METADATA_BASE_VALUE.saturating_sub(shift_count << 4);
    let encoded = existing.wrapping_add(metadata);
    data[FOW_METADATA_ENCODED_RANGE].copy_from_slice(&encoded.to_le_bytes());
    data[0] = FOW_METADATA_VERSION;

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
pub fn journey_bitmap_to_fwss_file<T: Write + Seek>(
    journey_bitmap: &JourneyBitmap,
    writer: &mut T,
) -> Result<()> {
    if journey_bitmap.is_empty() {
        bail!("No Fog of World data to export");
    }

    let mut zip = zip::ZipWriter::new(writer);
    // The official app writes Deflated zip entries, but every FWSS entry payload
    // below is already zlib-compressed. Store the outer zip entries as-is to
    // avoid spending CPU on a second compression pass that usually saves nothing.
    let options = zip::write::SimpleFileOptions::DEFAULT
        .compression_method(zip::CompressionMethod::Stored)
        .system(zip::System::Dos);

    let mut tile_keys = journey_bitmap.all_tile_keys().copied().collect::<Vec<_>>();
    tile_keys.sort();
    let mut pending_layers: BTreeMap<(i32, u16, u16), FoWSnapshotTile> = BTreeMap::new();
    let mut tile_index = [0_u8; FOW_SNAPSHOT_TILE_BITSET_SIZE];
    let mut total_area_square_meters = 0_u64;

    for tile_key in tile_keys {
        let Some(snapshot_tile) =
            journey_bitmap.peek_tile_without_updating_cache(&tile_key, |tile| match tile {
                Some(tile) if !tile.is_empty() => {
                    Some(FoWSnapshotTile::from_journey_tile(&tile_key, tile))
                }
                _ => None,
            })
        else {
            continue;
        };
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

        // The official snapshot index is a z9 512x512 tile bitset using
        // low-bit-first ordering within each byte.
        let tile_index_offset =
            ((tile_key.y as usize * MAP_WIDTH as usize) + tile_key.x as usize) / 8;
        tile_index[tile_index_offset] |= 1 << (tile_key.x % 8);

        let tile_area = fow_tile_row_area_square_meters(tile_key.y);
        total_area_square_meters += ((tile_area * snapshot_tile.count_pixels() as f64)
            / FOW_PIXELS_PER_BASE_TILE as f64) as u64;

        // FOW snapshots group all Model/* entries before Model/# entries;
        // we write each bitmap followed by its hash.
        zip.start_file(format!("Model/*/{bitmap_filename}"), options)?;
        zip.write_all(&serialize_fow_snapshot_bitmap_tile(&snapshot_tile)?)?;
        zip.start_file(format!("Model/#/{hash_filename}"), options)?;
        zip.write_all(&serialize_fow_snapshot_hash_tile(&snapshot_tile)?)?;

        pending_layers.insert(
            (
                snapshot_tile.coord.z,
                snapshot_tile.coord.y,
                snapshot_tile.coord.x,
            ),
            snapshot_tile,
        );
    }

    while !pending_layers.is_empty() {
        let mut next_layers = BTreeMap::new();
        for (_, tile) in pending_layers {
            if tile.coord.z <= FOW_SNAPSHOT_MAX_LAYER_Z
                && tile.coord.z >= FOW_SNAPSHOT_MIN_LAYER_Z
                && !tile.is_empty()
            {
                let filename = fow_snapshot_filename(
                    tile.coord.x,
                    tile.coord.y,
                    tile.coord.z,
                    FoWSnapshotFileType::Layer,
                );
                zip.start_file(format!("Model/~/{filename}"), options)?;
                zip.write_all(&serialize_fow_snapshot_layer_tile(&tile)?)?;
            }

            if tile.coord.z <= FOW_SNAPSHOT_MIN_LAYER_Z {
                break;
            }

            let parent_coord = tile.parent_coord();
            let parent_key = (parent_coord.z, parent_coord.y, parent_coord.x);
            next_layers
                .entry(parent_key)
                .or_insert_with(|| FoWSnapshotTile::empty(parent_coord))
                .merge_subtile(&tile);
        }

        pending_layers = next_layers;
    }

    zip.start_file(format!("Model/#/{FOW_SNAPSHOT_METADATA_FILENAME}"), options)?;
    zip.write_all(&fow_snapshot_metadata(total_area_square_meters)?)?;
    zip.start_file(
        format!("Model/#/{FOW_SNAPSHOT_TILE_INDEX_FILENAME}"),
        options,
    )?;
    zip.write_all(&serialize_fow_snapshot_tile_index(&tile_index)?)?;

    zip.finish()?;
    Ok(())
}

#[auto_context]
pub fn journey_vector_to_fwss_file<T: Write + Seek>(
    journey_vector: &JourneyVector,
    writer: &mut T,
) -> Result<()> {
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.merge_vector(journey_vector);
    journey_bitmap_to_fwss_file(&journey_bitmap, writer)
}
