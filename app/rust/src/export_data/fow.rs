use crate::journey_bitmap::{JourneyBitmap, TileKey, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH};
use crate::journey_vector::JourneyVector;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use md5::{Digest, Md5};
use std::collections::BTreeMap;
use std::io::{Seek, Write};

const FOW_FILENAME_MASK1: &str = "olhwjsktri";
const FOW_FILENAME_MASK2: &str = "eizxdwknmo";
const FOW_TILE_HEADER_SIZE: usize = (TILE_WIDTH * TILE_WIDTH * 2) as usize;
const FOW_BLOCK_EXTRA_DATA_SIZE: usize = 3;
const FOW_SNAPSHOT_TILE_Z: i32 = 9;
const FOW_SNAPSHOT_TILE_BITSET_SIZE: usize = (MAP_WIDTH * MAP_WIDTH / 8) as usize;
const FOW_SNAPSHOT_METADATA_SIZE: usize = 4012;
const FOW_EARTH_RADIUS_METERS: f64 = 6378137.0;

#[derive(Clone, Copy)]
enum FoWSnapshotFileType {
    Bitmap,
    Hash,
    Layer,
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

fn fow_snapshot_hash_block_data(bitmap: &[u8; BITMAP_SIZE]) -> [u8; FOW_BLOCK_EXTRA_DATA_SIZE] {
    let visited_count = bitmap.iter().map(|x| x.count_ones()).sum::<u32>();
    debug_assert!(visited_count <= 4096);
    let visited_count = visited_count as u16;
    [
        35,
        192 + ((visited_count >> 8) as u8),
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
                z: FOW_SNAPSHOT_TILE_Z,
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
    blocks: &BTreeMap<usize, [u8; BITMAP_SIZE]>,
    mut write_payload: F,
) -> Result<Vec<u8>>
where
    F: FnMut(&[u8; BITMAP_SIZE], &mut ZlibEncoder<Vec<u8>>) -> Result<()>,
{
    let mut header = vec![0_u8; FOW_TILE_HEADER_SIZE];
    let mut active_block_idx = 1;

    for &block_idx in blocks.keys() {
        let header_offset = block_idx * 2;
        header[header_offset] = (active_block_idx & 0xff) as u8;
        header[header_offset + 1] = (active_block_idx >> 8) as u8;
        active_block_idx += 1;
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
        encoder.write_all(&fow_block_extra_data(block))?;
        Ok(())
    })
}

fn serialize_fow_snapshot_hash_tile(tile: &FoWSnapshotTile) -> Result<Vec<u8>> {
    serialize_fow_snapshot_blocks(&tile.blocks, |block, encoder| {
        encoder.write_all(&fow_snapshot_hash_block_data(block))?;
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

    let mut tiles = journey_bitmap.iter_tiles().collect::<Vec<_>>();
    tiles.sort_by_key(|(tile_key, _)| (**tile_key).clone());
    let mut pending_layers: BTreeMap<(i32, u16, u16), FoWSnapshotTile> = BTreeMap::new();
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

        // The official snapshot index is a z9 512x512 tile bitset using
        // low-bit-first ordering within each byte.
        let tile_index_offset =
            ((tile_key.y as usize * MAP_WIDTH as usize) + tile_key.x as usize) / 8;
        tile_index[tile_index_offset] |= 1 << (tile_key.x % 8);

        let tile_area = fow_tile_row_area_square_meters(tile_key.y);
        total_area_square_meters +=
            ((tile_area * snapshot_tile.count_pixels() as f64) / 67_108_864.0) as u64;

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
            zip.start_file(format!("Model/~/{filename}"), options)?;
            zip.write_all(&serialize_fow_snapshot_layer_tile(&tile)?)?;
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

    zip.start_file("Model/#/01abfc750a", options)?;
    zip.write_all(&fow_snapshot_metadata(total_area_square_meters)?)?;
    zip.start_file("Model/#/3389dae361", options)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn bitmap_with_visited_count(count: usize) -> [u8; BITMAP_SIZE] {
        let mut bitmap = [0_u8; BITMAP_SIZE];
        let full_bytes = count / 8;
        bitmap[..full_bytes].fill(0xff);
        let remaining_bits = count % 8;
        if remaining_bits > 0 {
            bitmap[full_bytes] = 0xff << (8 - remaining_bits);
        }
        bitmap
    }

    #[test]
    fn fow_snapshot_hash_block_data_matches_eraser_format() {
        assert_eq!(
            fow_snapshot_hash_block_data(&bitmap_with_visited_count(0)),
            [35, 192, 0]
        );
        assert_eq!(
            fow_snapshot_hash_block_data(&bitmap_with_visited_count(100)),
            [35, 192, 100]
        );
        assert_eq!(
            fow_snapshot_hash_block_data(&bitmap_with_visited_count(4096)),
            [35, 208, 0]
        );
    }
}
