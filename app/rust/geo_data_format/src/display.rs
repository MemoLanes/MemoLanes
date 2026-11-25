//! Independently compressed, per-entity display geometry. Coordinates are signed
//! integer units of 0.00001°, and bounds are unwrapped degrees kept outside frames.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::{file_io::read_exact_at, GeoEntityId, Worldview};

const MAGIC: &[u8; 4] = b"MDSP";
/// Independent of the raster geo-data format and its provenance/version.
pub const DISPLAY_DATASET_VERSION: u32 = 3;
const HEADER_LEN: u64 = 72;
const INDEX_LEN: u64 = 41;
const BLOCK_LEN: u64 = 16;
const MAX_TABLE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_VERTICES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

#[derive(Debug, Clone)]
pub struct DisplayRecord {
    pub worldview: Worldview,
    pub entity_id: GeoEntityId,
    pub bounds: DisplayBounds,
    /// Polygon → rings (exterior first) → vertices, including closure.
    pub polygons: Vec<Vec<Vec<[i32; 2]>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayGeometry {
    pub bounds: DisplayBounds,
    pub polygons: Vec<Vec<Vec<[i32; 2]>>>,
}

#[derive(Clone, Copy)]
struct Span {
    offset: u64,
    len: u64,
}

#[derive(Clone, Copy)]
struct IndexEntry {
    worldview: Worldview,
    id: GeoEntityId,
    bounds: DisplayBounds,
    block: u32,
}

#[derive(Clone, Copy)]
struct BlockEntry {
    offset: u64, // relative to frame section
    len: u32,
    raw_len: u32,
}

pub struct DisplayAsset {
    file: File,
    provenance: BTreeMap<Worldview, [u8; 32]>,
    index: Vec<IndexEntry>,
    blocks: Vec<BlockEntry>,
    frames: Span,
}

fn worldview_byte(worldview: Worldview) -> u8 {
    match worldview {
        Worldview::Iso => 0,
        Worldview::Chn => 1,
        Worldview::Usa => 2,
    }
}

fn parse_worldview(byte: u8) -> Result<Worldview> {
    match byte {
        0 => Ok(Worldview::Iso),
        1 => Ok(Worldview::Chn),
        2 => Ok(Worldview::Usa),
        _ => bail!("display: invalid worldview {byte}"),
    }
}

fn check_bounds(b: DisplayBounds) -> Result<()> {
    ensure!(
        [b.west, b.south, b.east, b.north]
            .iter()
            .all(|v| v.is_finite())
            && b.west <= b.east
            && b.east - b.west <= 360.0
            && b.south <= b.north
            && b.south >= -90.0
            && b.north <= 90.0,
        "display: invalid bounds"
    );
    Ok(())
}

fn put_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}

fn get_varint(data: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..=63).step_by(7) {
        let byte = *data
            .get(*cursor)
            .ok_or_else(|| anyhow!("display: truncated varint"))?;
        *cursor += 1;
        ensure!(shift != 63 || byte <= 1, "display: varint overflow");
        value |= ((byte & 127) as u64) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    bail!("display: varint overflow")
}

fn put_signed(value: i64, out: &mut Vec<u8>) {
    put_varint(((value << 1) ^ (value >> 63)) as u64, out);
}

fn get_signed(data: &[u8], cursor: &mut usize) -> Result<i64> {
    let value = get_varint(data, cursor)?;
    Ok(((value >> 1) as i64) ^ (-((value & 1) as i64)))
}

fn encode(polygons: &[Vec<Vec<[i32; 2]>>]) -> Result<Vec<u8>> {
    ensure!(!polygons.is_empty(), "display: empty geometry");
    let mut raw = Vec::new();
    put_varint(polygons.len() as u64, &mut raw);
    let mut total = 0usize;
    for polygon in polygons {
        ensure!(!polygon.is_empty(), "display: empty polygon");
        put_varint(polygon.len() as u64, &mut raw);
        for ring in polygon {
            ensure!(
                ring.len() >= 4 && ring.first() == ring.last(),
                "display: ring not closed or too short"
            );
            let count = ring.len() - 1;
            total = total
                .checked_add(count)
                .context("display: vertex count overflow")?;
            ensure!(total <= MAX_VERTICES, "display: too many vertices");
            put_varint(count as u64, &mut raw);
            let (mut x, mut y) = (0i64, 0i64);
            for &[next_x, next_y] in &ring[..count] {
                put_signed(next_x as i64 - x, &mut raw);
                put_signed(next_y as i64 - y, &mut raw);
                x = next_x as i64;
                y = next_y as i64;
            }
        }
    }
    ensure!(raw.len() <= MAX_FRAME_BYTES, "display: frame too large");
    Ok(raw)
}

fn decode(raw: &[u8]) -> Result<Vec<Vec<Vec<[i32; 2]>>>> {
    let mut cursor = 0;
    let polygon_count = get_varint(raw, &mut cursor)?;
    ensure!(
        polygon_count > 0 && polygon_count <= raw.len() as u64,
        "display: invalid polygon count"
    );
    let mut polygons = Vec::new();
    let mut total = 0usize;
    for _ in 0..polygon_count {
        let ring_count = get_varint(raw, &mut cursor)?;
        ensure!(
            ring_count > 0 && ring_count <= (raw.len() - cursor) as u64,
            "display: invalid ring count"
        );
        let mut rings = Vec::new();
        for _ in 0..ring_count {
            let count = get_varint(raw, &mut cursor)?;
            ensure!(
                count >= 3 && count <= ((raw.len() - cursor) / 2) as u64,
                "display: invalid vertex count"
            );
            total = total
                .checked_add(count as usize)
                .context("display: vertex count overflow")?;
            ensure!(total <= MAX_VERTICES, "display: too many vertices");
            let mut ring = Vec::with_capacity(count as usize + 1);
            let (mut x, mut y) = (0i64, 0i64);
            for _ in 0..count {
                x = x
                    .checked_add(get_signed(raw, &mut cursor)?)
                    .context("display: x overflow")?;
                y = y
                    .checked_add(get_signed(raw, &mut cursor)?)
                    .context("display: y overflow")?;
                ring.push([
                    i32::try_from(x).context("display: x out of range")?,
                    i32::try_from(y).context("display: y out of range")?,
                ]);
            }
            ring.push(ring[0]);
            rings.push(ring);
        }
        polygons.push(rings);
    }
    ensure!(cursor == raw.len(), "display: trailing frame data");
    Ok(polygons)
}

fn put_span(out: &mut Vec<u8>, offset: usize, len: usize) -> Result<()> {
    out.extend_from_slice(&u64::try_from(offset)?.to_le_bytes());
    out.extend_from_slice(&u64::try_from(len)?.to_le_bytes());
    Ok(())
}

/// Write sorted tables and deduplicate *raw* blocks before compression.
pub fn write_display_asset(
    worldviews: &[(Worldview, [u8; 32])],
    records: &[DisplayRecord],
) -> Result<Vec<u8>> {
    let mut manifest = BTreeMap::new();
    for &(worldview, hash) in worldviews {
        ensure!(
            manifest.insert(worldview, hash).is_none(),
            "display: duplicate worldview"
        );
    }
    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|r| (r.worldview, r.entity_id));
    let mut index = Vec::new();
    let mut block_bytes = Vec::new();
    let mut frames = Vec::new();
    let mut seen = BTreeMap::<Vec<u8>, u32>::new();
    let mut previous = None;
    for record in sorted {
        let key = (record.worldview, record.entity_id);
        ensure!(previous != Some(key), "display: duplicate entity");
        previous = Some(key);
        ensure!(
            manifest.contains_key(&record.worldview),
            "display: worldview absent from manifest"
        );
        check_bounds(record.bounds)?;
        let raw = encode(&record.polygons)?;
        let block = if let Some(&id) = seen.get(&raw) {
            id
        } else {
            let compressed = zstd::bulk::compress(&raw, 3)?;
            ensure!(
                compressed.len() <= MAX_FRAME_BYTES,
                "display: compressed frame too large"
            );
            let id = u32::try_from(block_bytes.len() / BLOCK_LEN as usize)?;
            block_bytes.extend_from_slice(&u64::try_from(frames.len())?.to_le_bytes());
            block_bytes.extend_from_slice(&u32::try_from(compressed.len())?.to_le_bytes());
            block_bytes.extend_from_slice(&u32::try_from(raw.len())?.to_le_bytes());
            frames.extend_from_slice(&compressed);
            seen.insert(raw, id);
            id
        };
        index.push(worldview_byte(record.worldview));
        index.extend_from_slice(&record.entity_id.0.to_le_bytes());
        for value in [
            record.bounds.west,
            record.bounds.south,
            record.bounds.east,
            record.bounds.north,
        ] {
            index.extend_from_slice(&value.to_le_bytes());
        }
        index.extend_from_slice(&block.to_le_bytes());
    }
    ensure!(
        index.len() as u64 <= MAX_TABLE_BYTES && block_bytes.len() as u64 <= MAX_TABLE_BYTES,
        "display: table too large"
    );
    let mut manifest_bytes = Vec::new();
    for (worldview, hash) in manifest {
        manifest_bytes.push(worldview_byte(worldview));
        manifest_bytes.extend_from_slice(&hash);
    }
    let mut result = Vec::from(&MAGIC[..]);
    result.extend_from_slice(&DISPLAY_DATASET_VERSION.to_le_bytes());
    let mut offset = HEADER_LEN as usize;
    for section in [&manifest_bytes, &index, &block_bytes, &frames] {
        put_span(&mut result, offset, section.len())?;
        offset = offset
            .checked_add(section.len())
            .context("display: file size overflow")?;
    }
    for section in [&manifest_bytes, &index, &block_bytes, &frames] {
        result.extend_from_slice(section);
    }
    Ok(result)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

fn read_section(file: &mut File, span: Span) -> Result<Vec<u8>> {
    let mut bytes = vec![0; usize::try_from(span.len)?];
    file.seek(SeekFrom::Start(span.offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

impl DisplayAsset {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let size = file.metadata()?.len();
        ensure!(size >= HEADER_LEN, "display: short header");
        let mut header = [0; HEADER_LEN as usize];
        file.read_exact(&mut header)?;
        ensure!(
            &header[..4] == MAGIC && u32_at(&header, 4) == DISPLAY_DATASET_VERSION,
            "display: incompatible header"
        );
        let mut spans = Vec::new();
        let mut expected = HEADER_LEN;
        for at in (8..HEADER_LEN as usize).step_by(16) {
            let span = Span {
                offset: u64_at(&header, at),
                len: u64_at(&header, at + 8),
            };
            ensure!(
                span.offset == expected,
                "display: overlapping or noncontiguous sections"
            );
            expected = span
                .offset
                .checked_add(span.len)
                .context("display: section overflow")?;
            ensure!(expected <= size, "display: section past end of file");
            spans.push(span);
        }
        ensure!(expected == size, "display: trailing data");
        ensure!(
            spans[0].len <= 33 * 3
                && spans[0].len % 33 == 0
                && spans[1].len <= MAX_TABLE_BYTES
                && spans[1].len % INDEX_LEN == 0
                && spans[2].len <= MAX_TABLE_BYTES
                && spans[2].len % BLOCK_LEN == 0,
            "display: invalid table lengths"
        );
        let manifest = read_section(&mut file, spans[0])?;
        let mut provenance = BTreeMap::new();
        for row in manifest.as_chunks::<33>().0 {
            let worldview = parse_worldview(row[0])?;
            ensure!(
                provenance.insert(worldview, row[1..].try_into()?).is_none(),
                "display: duplicate worldview"
            );
        }
        let table = read_section(&mut file, spans[1])?;
        let mut index = Vec::new();
        let mut last = None;
        for row in table.as_chunks::<{ INDEX_LEN as usize }>().0 {
            let worldview = parse_worldview(row[0])?;
            let id = GeoEntityId(u32_at(row, 1));
            let key = (worldview, id);
            ensure!(
                last.is_none_or(|previous| previous < key),
                "display: unsorted or duplicate index"
            );
            ensure!(
                provenance.contains_key(&worldview),
                "display: unlisted worldview"
            );
            last = Some(key);
            let bounds = DisplayBounds {
                west: f64_at(row, 5),
                south: f64_at(row, 13),
                east: f64_at(row, 21),
                north: f64_at(row, 29),
            };
            check_bounds(bounds)?;
            index.push(IndexEntry {
                worldview,
                id,
                bounds,
                block: u32_at(row, 37),
            });
        }
        let table = read_section(&mut file, spans[2])?;
        let mut blocks = Vec::new();
        let mut end = 0;
        for row in table.as_chunks::<{ BLOCK_LEN as usize }>().0 {
            let entry = BlockEntry {
                offset: u64_at(row, 0),
                len: u32_at(row, 8),
                raw_len: u32_at(row, 12),
            };
            ensure!(
                entry.offset == end
                    && entry.len > 0
                    && entry.len as usize <= MAX_FRAME_BYTES
                    && entry.raw_len > 0
                    && entry.raw_len as usize <= MAX_FRAME_BYTES,
                "display: invalid block length or overlap"
            );
            end = entry
                .offset
                .checked_add(entry.len as u64)
                .context("display: block overflow")?;
            ensure!(end <= spans[3].len, "display: block past frames");
            blocks.push(entry);
        }
        ensure!(end == spans[3].len, "display: unindexed frame bytes");
        ensure!(
            index
                .iter()
                .all(|entry| (entry.block as usize) < blocks.len()),
            "display: bad block reference"
        );
        Ok(Self {
            file,
            provenance,
            index,
            blocks,
            frames: spans[3],
        })
    }

    pub fn raster_provenance(&self, worldview: Worldview) -> Option<[u8; 32]> {
        self.provenance.get(&worldview).copied()
    }

    /// Only the requested compressed frame is read and decompressed.
    pub fn get(&self, worldview: Worldview, id: GeoEntityId) -> Result<Option<DisplayGeometry>> {
        let Ok(row) = self
            .index
            .binary_search_by_key(&(worldview, id), |entry| (entry.worldview, entry.id))
        else {
            return Ok(None);
        };
        let entry = self.index[row];
        let block = self.blocks[entry.block as usize];
        let mut compressed = vec![0; block.len as usize];
        read_exact_at(
            &self.file,
            &mut compressed,
            self.frames.offset + block.offset,
        )?;
        let raw = zstd::bulk::decompress(&compressed, block.raw_len as usize)
            .context("display: decompressing entity frame")?;
        ensure!(
            raw.len() == block.raw_len as usize,
            "display: wrong decompressed length"
        );
        Ok(Some(DisplayGeometry {
            bounds: entry.bounds,
            polygons: decode(&raw)?,
        }))
    }
}
