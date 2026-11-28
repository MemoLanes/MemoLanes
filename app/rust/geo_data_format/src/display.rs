//! The display section holds a sorted entity index and one zstd frame per entity.
//! Both header spans are relative to the display section. Each index row stores
//! its frame offset relative to the frames span and its compressed and raw lengths.
//! Bounds are unwrapped degrees kept outside frames. A frame starts with the polygon count, or a
//! zero marker followed by the polygon count for exact-degree coordinates.
//! Both use signed delta varints. Exact coordinates delta-encode the f64 bits
//! with wrapping arithmetic. Ring closure is implicit in both encodings.
use std::fs::File;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::{file_io::read_exact_at, GeoEntityId, Section, Worldview};

const DISPLAY_HEADER_LEN: usize = 2 * 16;
const INDEX_LEN: usize = 52;
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_VERTICES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Polygon → rings (exterior first) → vertices, including closure.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayPolygons {
    /// Signed units of 0.00001°.
    Grid(Vec<Vec<Vec<[i32; 2]>>>),
    /// Finite degrees, with bit-identical first and last vertices in each ring.
    Exact(Vec<Vec<Vec<[f64; 2]>>>),
}

impl DisplayPolygons {
    pub fn into_degrees(self) -> Vec<Vec<Vec<[f64; 2]>>> {
        match self {
            Self::Grid(polygons) => polygons
                .into_iter()
                .map(|polygon| {
                    polygon
                        .into_iter()
                        .map(|ring| {
                            ring.into_iter()
                                .map(|point| point.map(|v| f64::from(v) / 100_000.0))
                                .collect()
                        })
                        .collect()
                })
                .collect(),
            Self::Exact(polygons) => polygons,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayRecord {
    pub worldview: Worldview,
    pub entity_id: GeoEntityId,
    pub bounds: DisplayBounds,
    pub polygons: DisplayPolygons,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayGeometry {
    pub bounds: DisplayBounds,
    pub polygons: DisplayPolygons,
}

#[derive(Debug)]
struct IndexEntry {
    id: GeoEntityId,
    bounds: DisplayBounds,
    file_offset: u64,
    compressed_len: u32,
    raw_len: u32,
}

#[derive(Debug)]
pub(crate) struct DisplayIndex {
    entries: Vec<IndexEntry>,
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

fn finite_coordinate(value: f64) -> Result<f64> {
    ensure!(value.is_finite(), "display: non-finite coordinate");
    Ok(value)
}

fn encode(polygons: &DisplayPolygons) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    match polygons {
        DisplayPolygons::Grid(polygons) => {
            encode_polygons(polygons, &mut raw, |v| Ok(i64::from(v)))?
        }
        DisplayPolygons::Exact(polygons) => {
            put_varint(0, &mut raw);
            encode_polygons(polygons, &mut raw, |v| {
                Ok(finite_coordinate(v)?.to_bits() as i64)
            })?;
        }
    }
    Ok(raw)
}

fn encode_polygons<T: Copy>(
    polygons: &[Vec<Vec<[T; 2]>>],
    raw: &mut Vec<u8>,
    coordinate_bits: impl Fn(T) -> Result<i64>,
) -> Result<()> {
    ensure!(!polygons.is_empty(), "display: empty geometry");
    let point_bits =
        |[x, y]: [T; 2]| -> Result<[i64; 2]> { Ok([coordinate_bits(x)?, coordinate_bits(y)?]) };
    put_varint(polygons.len() as u64, raw);
    let mut total = 0usize;
    for polygon in polygons {
        ensure!(!polygon.is_empty(), "display: empty polygon");
        put_varint(polygon.len() as u64, raw);
        for ring in polygon {
            ensure!(ring.len() >= 4, "display: ring not closed or too short");
            let count = ring.len() - 1;
            // The closing point is implicit, so even its sign bits must match.
            ensure!(
                point_bits(ring[0])? == point_bits(ring[count])?,
                "display: ring not closed or too short"
            );
            total = total
                .checked_add(count)
                .context("display: vertex count overflow")?;
            ensure!(total <= MAX_VERTICES, "display: too many vertices");
            put_varint(count as u64, raw);
            let (mut x, mut y) = (0i64, 0i64);
            for &point in &ring[..count] {
                let [next_x, next_y] = point_bits(point)?;
                put_signed(next_x.wrapping_sub(x), raw);
                put_signed(next_y.wrapping_sub(y), raw);
                ensure!(raw.len() <= MAX_FRAME_BYTES, "display: frame too large");
                x = next_x;
                y = next_y;
            }
        }
    }
    Ok(())
}

fn decode(raw: &[u8]) -> Result<DisplayPolygons> {
    let mut cursor = 0;
    let polygon_count = get_varint(raw, &mut cursor)?;
    if polygon_count == 0 {
        let polygon_count = get_varint(raw, &mut cursor)?;
        decode_polygons(raw, cursor, polygon_count, |previous, delta| {
            *previous = previous.wrapping_add(delta);
            finite_coordinate(f64::from_bits(*previous as u64))
        })
        .map(DisplayPolygons::Exact)
    } else {
        decode_polygons(raw, cursor, polygon_count, |previous, delta| {
            *previous = previous
                .checked_add(delta)
                .context("display: grid coordinate overflow")?;
            i32::try_from(*previous).context("display: grid coordinate out of range")
        })
        .map(DisplayPolygons::Grid)
    }
}

fn decode_polygons<T: Copy>(
    raw: &[u8],
    mut cursor: usize,
    polygon_count: u64,
    coordinate: impl Fn(&mut i64, i64) -> Result<T>,
) -> Result<Vec<Vec<Vec<[T; 2]>>>> {
    ensure!(
        polygon_count > 0 && polygon_count <= (raw.len() - cursor) as u64,
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
                ring.push([
                    coordinate(&mut x, get_signed(raw, &mut cursor)?)?,
                    coordinate(&mut y, get_signed(raw, &mut cursor)?)?,
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

/// Write one independently readable frame per entity in ID order.
pub(crate) fn write_display_section(
    worldview_id: &str,
    records: &[DisplayRecord],
) -> Result<Vec<u8>> {
    ensure!(
        records.len() as u64 <= MAX_INDEX_BYTES / INDEX_LEN as u64,
        "display: table too large"
    );
    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|r| r.entity_id);
    let mut compressor = zstd::bulk::Compressor::new(3)?;
    let mut index = Vec::with_capacity(records.len() * INDEX_LEN);
    let mut frames = Vec::new();
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
        let compressed = compressor.compress(&raw)?;
        ensure!(
            compressed.len() <= MAX_FRAME_BYTES,
            "display: compressed frame too large"
        );
        index.extend_from_slice(&record.entity_id.0.to_le_bytes());
        for value in [
            record.bounds.west,
            record.bounds.south,
            record.bounds.east,
            record.bounds.north,
        ] {
            index.extend_from_slice(&value.to_le_bytes());
        }
        index.extend_from_slice(&u64::try_from(frames.len())?.to_le_bytes());
        index.extend_from_slice(&u32::try_from(compressed.len())?.to_le_bytes());
        index.extend_from_slice(&u32::try_from(raw.len())?.to_le_bytes());
        frames.extend_from_slice(&compressed);
    }
    let frames_offset = DISPLAY_HEADER_LEN + index.len();
    let mut result = Vec::with_capacity(
        frames_offset
            .checked_add(frames.len())
            .context("display: section size overflow")?,
    );
    put_span(&mut result, DISPLAY_HEADER_LEN, index.len())?;
    put_span(&mut result, frames_offset, frames.len())?;
    result.extend_from_slice(&index);
    result.extend_from_slice(&frames);
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

impl DisplayIndex {
    pub(crate) fn read(file: &File, section: Section) -> Result<Self> {
        ensure!(section.len >= DISPLAY_HEADER_LEN, "display: short header");
        let mut header = [0; DISPLAY_HEADER_LEN];
        read_exact_at(file, &mut header, section.off as u64).context("display: reading header")?;
        let index_offset = u64_at(&header, 0);
        let index_len = u64_at(&header, 8);
        let frames_offset = u64_at(&header, 16);
        let frames_len = u64_at(&header, 24);
        ensure!(
            index_offset == DISPLAY_HEADER_LEN as u64
                && index_len <= MAX_INDEX_BYTES
                && index_len.is_multiple_of(INDEX_LEN as u64),
            "display: invalid index span"
        );
        ensure!(
            frames_offset == index_offset + index_len
                && frames_offset.checked_add(frames_len) == Some(section.len as u64),
            "display: invalid frames span"
        );
        let mut table = vec![0; usize::try_from(index_len)?];
        read_exact_at(file, &mut table, section.off as u64 + index_offset)
            .context("display: reading index")?;
        let mut entries = Vec::with_capacity(table.len() / INDEX_LEN);
        let mut last = None;
        let mut end = 0;
        for row in table.as_chunks::<INDEX_LEN>().0 {
            let id = GeoEntityId(u32_at(row, 0));
            ensure!(
                last.is_none_or(|previous| previous < id),
                "display: unsorted or duplicate index at entity ID {}",
                id.0
            );
            last = Some(id);
            let bounds = DisplayBounds {
                west: f64_at(row, 4),
                south: f64_at(row, 12),
                east: f64_at(row, 20),
                north: f64_at(row, 28),
            };
            check_bounds(bounds)?;
            let offset = u64_at(row, 36);
            let compressed_len = u32_at(row, 44);
            let raw_len = u32_at(row, 48);
            ensure!(
                offset == end
                    && compressed_len > 0
                    && compressed_len as usize <= MAX_FRAME_BYTES
                    && raw_len > 0
                    && raw_len as usize <= MAX_FRAME_BYTES,
                "display: invalid frame offset or length for entity ID {}",
                id.0
            );
            end = offset
                .checked_add(u64::from(compressed_len))
                .context("display: frame offset overflow")?;
            ensure!(end <= frames_len, "display: frame past end of section");
            entries.push(IndexEntry {
                id,
                bounds,
                file_offset: section.off as u64 + frames_offset + offset,
                compressed_len,
                raw_len,
            });
        }
        ensure!(end == frames_len, "display: unindexed frame bytes");
        Ok(Self { entries })
    }

    pub(crate) fn ids(&self) -> Vec<GeoEntityId> {
        self.entries.iter().map(|entry| entry.id).collect()
    }

    /// Only the requested compressed frame is read and decompressed.
    pub(crate) fn get(&self, file: &File, id: GeoEntityId) -> Result<Option<DisplayGeometry>> {
        let Ok(row) = self.entries.binary_search_by_key(&id, |entry| entry.id) else {
            return Ok(None);
        };
        self.entries[row].read_geometry(file).map(Some)
    }

    pub(crate) fn validate_frames(
        &self,
        file: &File,
        mut validate: impl FnMut(GeoEntityId, DisplayGeometry) -> Result<()>,
    ) -> Result<()> {
        for entry in &self.entries {
            let geometry = entry
                .read_geometry(file)
                .with_context(|| format!("display: decoding entity ID {}", entry.id.0))?;
            validate(entry.id, geometry)?;
        }
        Ok(())
    }
}

impl IndexEntry {
    fn read_geometry(&self, file: &File) -> Result<DisplayGeometry> {
        let mut compressed = vec![0; self.compressed_len as usize];
        read_exact_at(file, &mut compressed, self.file_offset)
            .context("display: reading entity frame")?;
        let raw = zstd::bulk::decompress(&compressed, self.raw_len as usize)
            .context("display: decompressing entity frame")?;
        ensure!(
            raw.len() == self.raw_len as usize,
            "display: wrong decompressed length"
        );
        Ok(DisplayGeometry {
            bounds: self.bounds,
            polygons: decode(&raw)?,
        })
    }
}
