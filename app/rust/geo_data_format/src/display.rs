//! The display section holds the entity index, block table and checksummed zstd
//! frames. Each span in the section header is relative to the display section;
//! each block offset is relative to the frames span. Coordinates are signed
//! integer units of 0.00001°; bounds are unwrapped degrees kept outside frames.
use std::collections::BTreeMap;
use std::fs::File;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::{file_io::read_exact_at, GeoEntityId, Section, Worldview};

const DISPLAY_HEADER_LEN: usize = 3 * 16;
const INDEX_LEN: usize = 40;
const BLOCK_LEN: usize = 16;
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

#[derive(Debug, Clone, Copy)]
struct Span {
    offset: u64,
    len: u64,
}

#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    id: GeoEntityId,
    bounds: DisplayBounds,
    block: u32,
}

#[derive(Debug, Clone, Copy)]
struct BlockEntry {
    offset: u64, // relative to frame section
    len: u32,
    raw_len: u32,
}

#[derive(Debug)]
pub(crate) struct DisplayIndex {
    index: Vec<IndexEntry>,
    blocks: Vec<BlockEntry>,
    frames_offset: u64, // absolute file offset, validated against the display section
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
pub(crate) fn write_display_section(
    worldview_id: &str,
    records: &[DisplayRecord],
) -> Result<Vec<u8>> {
    ensure!(
        records.len() as u64 <= MAX_TABLE_BYTES / INDEX_LEN as u64,
        "display: table too large"
    );
    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|r| r.entity_id);
    let mut compressor = zstd::bulk::Compressor::new(3)?;
    compressor.set_parameter(zstd::zstd_safe::CParameter::ChecksumFlag(true))?;
    let mut index = Vec::new();
    let mut block_bytes = Vec::new();
    let mut frames = Vec::new();
    let mut seen = BTreeMap::<Vec<u8>, u32>::new();
    let mut previous = None;
    for record in sorted {
        ensure!(
            record.worldview.spec().id == worldview_id,
            "display: record worldview {:?} does not match asset worldview {worldview_id}",
            record.worldview
        );
        ensure!(
            previous != Some(record.entity_id),
            "display: duplicate entity"
        );
        previous = Some(record.entity_id);
        check_bounds(record.bounds)?;
        let raw = encode(&record.polygons)?;
        let block = if let Some(&id) = seen.get(&raw) {
            id
        } else {
            let compressed = compressor.compress(&raw)?;
            ensure!(
                compressed.len() <= MAX_FRAME_BYTES,
                "display: compressed frame too large"
            );
            let id = u32::try_from(block_bytes.len() / BLOCK_LEN)?;
            block_bytes.extend_from_slice(&u64::try_from(frames.len())?.to_le_bytes());
            block_bytes.extend_from_slice(&u32::try_from(compressed.len())?.to_le_bytes());
            block_bytes.extend_from_slice(&u32::try_from(raw.len())?.to_le_bytes());
            frames.extend_from_slice(&compressed);
            seen.insert(raw, id);
            id
        };
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
    let mut result = Vec::with_capacity(DISPLAY_HEADER_LEN);
    let mut offset = DISPLAY_HEADER_LEN;
    let sections = [&index, &block_bytes, &frames];
    for section in sections {
        put_span(&mut result, offset, section.len())?;
        offset = offset
            .checked_add(section.len())
            .context("display: section size overflow")?;
    }
    result.reserve(offset - DISPLAY_HEADER_LEN);
    for section in sections {
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

fn read_table(file: &File, section: Section, span: Span) -> Result<Vec<u8>> {
    let mut bytes = vec![0; usize::try_from(span.len)?];
    read_exact_at(file, &mut bytes, section.off as u64 + span.offset)
        .context("display: reading table")?;
    Ok(bytes)
}

impl DisplayIndex {
    pub(crate) fn read(file: &File, section: Section) -> Result<Self> {
        ensure!(section.len >= DISPLAY_HEADER_LEN, "display: short header");
        let mut header = [0; DISPLAY_HEADER_LEN];
        read_exact_at(file, &mut header, section.off as u64).context("display: reading header")?;
        let mut expected = DISPLAY_HEADER_LEN as u64;
        let mut spans = [Span { offset: 0, len: 0 }; 3];
        for (row, span) in header.as_chunks::<16>().0.iter().zip(&mut spans) {
            *span = Span {
                offset: u64_at(row, 0),
                len: u64_at(row, 8),
            };
            ensure!(
                span.offset == expected,
                "display: overlapping or noncontiguous sections"
            );
            expected = span
                .offset
                .checked_add(span.len)
                .context("display: section overflow")?;
            ensure!(
                expected <= section.len as u64,
                "display: span out of bounds"
            );
        }
        ensure!(expected == section.len as u64, "display: trailing data");
        let [index_span, blocks_span, frames_span] = spans;
        ensure!(
            index_span.len <= MAX_TABLE_BYTES
                && index_span.len.is_multiple_of(INDEX_LEN as u64)
                && blocks_span.len <= MAX_TABLE_BYTES
                && blocks_span.len.is_multiple_of(BLOCK_LEN as u64),
            "display: invalid table lengths"
        );
        let table = read_table(file, section, index_span)?;
        let mut index = Vec::with_capacity(table.len() / INDEX_LEN);
        let mut last = None;
        for row in table.as_chunks::<INDEX_LEN>().0 {
            let id = GeoEntityId(u32_at(row, 0));
            ensure!(
                last.is_none_or(|previous| previous < id),
                "display: unsorted or duplicate index"
            );
            last = Some(id);
            let bounds = DisplayBounds {
                west: f64_at(row, 4),
                south: f64_at(row, 12),
                east: f64_at(row, 20),
                north: f64_at(row, 28),
            };
            check_bounds(bounds)?;
            index.push(IndexEntry {
                id,
                bounds,
                block: u32_at(row, 36),
            });
        }
        let table = read_table(file, section, blocks_span)?;
        let mut blocks = Vec::with_capacity(table.len() / BLOCK_LEN);
        let mut end = 0;
        for row in table.as_chunks::<BLOCK_LEN>().0 {
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
            ensure!(end <= frames_span.len, "display: block past frames");
            blocks.push(entry);
        }
        ensure!(end == frames_span.len, "display: unindexed frame bytes");
        ensure!(
            index
                .iter()
                .all(|entry| (entry.block as usize) < blocks.len()),
            "display: bad block reference"
        );
        Ok(Self {
            index,
            blocks,
            frames_offset: section.off as u64 + frames_span.offset,
        })
    }

    /// Only the requested compressed frame is read and decompressed.
    pub(crate) fn get(&self, file: &File, id: GeoEntityId) -> Result<Option<DisplayGeometry>> {
        let Ok(row) = self.index.binary_search_by_key(&id, |entry| entry.id) else {
            return Ok(None);
        };
        let entry = self.index[row];
        let raw = self.read_frame(file, self.blocks[entry.block as usize])?;
        Ok(Some(DisplayGeometry {
            bounds: entry.bounds,
            polygons: decode(&raw)?,
        }))
    }

    pub(crate) fn validate_frames(&self, file: &File) -> Result<()> {
        for &block in &self.blocks {
            decode(&self.read_frame(file, block)?)?;
        }
        Ok(())
    }

    fn read_frame(&self, file: &File, block: BlockEntry) -> Result<Vec<u8>> {
        let mut compressed = vec![0; block.len as usize];
        read_exact_at(file, &mut compressed, self.frames_offset + block.offset)
            .context("display: reading entity frame")?;
        let raw = zstd::bulk::decompress(&compressed, block.raw_len as usize)
            .context("display: decompressing entity frame")?;
        ensure!(
            raw.len() == block.raw_len as usize,
            "display: wrong decompressed length"
        );
        Ok(raw)
    }
}
