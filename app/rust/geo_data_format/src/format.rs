//! Sectioned on-disk geo data format.
//!
//! Layout: `Header(76 B) | Meta | TileIndex | BorderOffsets | BorderBlobs | Display`.
//! All integers little-endian. Border tiles are stored already
//! `PackedTile`-compressed. [`GeoData::open`] keeps the tile index (sparse)
//! and the compressed meta section in memory; border offsets and blobs stay
//! on disk. Display tables are read on the first geometry lookup, and only
//! the requested frame is decoded. A zero-length display span is raster-only.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::OnceLock;

use bitmap_geometry::BitmapPlane;
use serde::{Deserialize, Serialize};

use crate::{
    display::{write_display_section, DisplayIndex},
    file_io::read_exact_at,
    DisplayGeometry, DisplayRecord, GeoEntity, GeoEntityId, PackedTile, TileMembership,
    ALL_PLANE_TILE_COUNT, MAGIC, PROVENANCE_HASH_END,
};

/// magic(4) + provenance_hash(32) + 5 sections × (u32 offset, u32 len).
pub const HEADER_LEN: usize = PROVENANCE_HASH_END + 5 * 8;
const META_ZSTD_LEVEL: i32 = 19;
const TILE_INDEX_ZSTD_LEVEL: i32 = 19;

#[derive(Serialize, Deserialize)]
struct MetaSection {
    entities: Vec<GeoEntity>,
    /// The single worldview this asset represents (its [`crate::Worldview`]
    /// id). Makes the `.bin` self-describing so a load can reject a bin served
    /// under the wrong worldview.
    worldview_id: String,
}

/// Tile classification as read back: `Border` carries the index into
/// `GeoData::border_blobs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileEntry {
    Single(GeoEntityId),
    Border(u32),
    None,
}

const TILE_BORDER_BIT: u32 = 1 << 31;
const TILE_PAYLOAD_MAX: u32 = TILE_BORDER_BIT - 1;
const TILES_PER_WORD: usize = 64;

fn check_tile_payload(what: &str, value: u32) -> anyhow::Result<()> {
    anyhow::ensure!(
        value <= TILE_PAYLOAD_MAX,
        "geo_data: {what} {value} exceeds the tile index range ({TILE_PAYLOAD_MAX})"
    );
    Ok(())
}

#[derive(Debug)]
pub struct TileIndex {
    present: Box<[u64]>,
    rank: Box<[u32]>,
    entries: Box<[u32]>,
}

impl TileIndex {
    fn from_raw(tile_raw: &[u8]) -> anyhow::Result<Self> {
        let words = ALL_PLANE_TILE_COUNT / TILES_PER_WORD;
        let mut present = vec![0u64; words];
        let mut rank = vec![0u32; words];
        let mut entries = Vec::new();
        for (i, raw) in tile_raw.as_chunks::<5>().0.iter().enumerate() {
            if i % TILES_PER_WORD == 0 {
                rank[i / TILES_PER_WORD] = entries.len() as u32;
            }
            let payload = read_u32(raw, 1);
            let packed = match raw[0] {
                0 => continue,
                1 => {
                    check_tile_payload("entity id", payload)?;
                    payload
                }
                3 => {
                    check_tile_payload("border blob index", payload)?;
                    TILE_BORDER_BIT | payload
                }
                t => anyhow::bail!("geo_data: bad tile tag {t}"),
            };
            present[i / TILES_PER_WORD] |= 1 << (i % TILES_PER_WORD);
            entries.push(packed);
        }
        Ok(TileIndex {
            present: present.into_boxed_slice(),
            rank: rank.into_boxed_slice(),
            entries: entries.into_boxed_slice(),
        })
    }

    pub fn get(&self, idx: usize) -> TileEntry {
        let word = self.present[idx / TILES_PER_WORD];
        let bit = 1u64 << (idx % TILES_PER_WORD);
        if word & bit == 0 {
            return TileEntry::None;
        }
        let below = (word & (bit - 1)).count_ones() as usize;
        let v = self.entries[self.rank[idx / TILES_PER_WORD] as usize + below];
        if v & TILE_BORDER_BIT != 0 {
            TileEntry::Border(v & !TILE_BORDER_BIT)
        } else {
            TileEntry::Single(GeoEntityId(v))
        }
    }

    fn max_border_index(&self) -> Option<u32> {
        self.entries
            .iter()
            .filter(|v| *v & TILE_BORDER_BIT != 0)
            .map(|v| v & !TILE_BORDER_BIT)
            .max()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub off: usize,
    pub len: usize,
}

#[derive(Debug)]
pub struct BorderBlobs {
    file: File,
    offsets: Section,
    region: Section,
}

impl BorderBlobs {
    fn count(&self) -> usize {
        self.offsets.len / 8
    }

    pub fn get(&self, i: u32) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!((i as usize) < self.count(), "geo_data: no border blob {i}");
        let mut span = [0u8; 8];
        read_exact_at(
            &self.file,
            &mut span,
            (self.offsets.off + i as usize * 8) as u64,
        )
        .map_err(|e| anyhow::anyhow!("geo_data: reading span of border blob {i}: {e}"))?;
        let (off, len) = (read_u32(&span, 0), read_u32(&span, 4));
        anyhow::ensure!(
            off.checked_add(len)
                .is_some_and(|end| end as usize <= self.region.len),
            "geo_data: border blob {i} out of bounds"
        );
        let mut buf = vec![0u8; len as usize];
        read_exact_at(
            &self.file,
            &mut buf,
            (self.region.off + off as usize) as u64,
        )
        .map_err(|e| anyhow::anyhow!("geo_data: reading border blob {i}: {e}"))?;
        Ok(buf)
    }
}

#[derive(Debug)]
pub struct GeoData {
    /// The worldview id this asset represents (see [`MetaSection::worldview_id`]).
    pub worldview_id: String,
    pub tile_index: TileIndex,
    pub border_blobs: BorderBlobs,
    pub provenance_hash: [u8; 32],
    meta_compressed: Box<[u8]>,
    display: Section,
    display_index: OnceLock<anyhow::Result<DisplayIndex>>,
}

impl GeoData {
    pub fn entities(&self) -> anyhow::Result<Vec<GeoEntity>> {
        Ok(decode_meta(&self.meta_compressed)?.entities)
    }

    /// Validate all embedded outlines for build-cache reuse, not runtime activation.
    pub fn validate_display(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.display.len != 0, "geo_data: no display section");
        let file = &self.border_blobs.file;
        DisplayIndex::read(file, self.display)?.validate_frames(file)
    }

    /// Read this entity's display geometry from the same open file as the border blobs.
    /// Missing geometry (including raster-only files) is not an error.
    pub fn display_geometry(&self, id: GeoEntityId) -> anyhow::Result<Option<DisplayGeometry>> {
        if self.display.len == 0 {
            return Ok(None);
        }
        let file = &self.border_blobs.file;
        match self
            .display_index
            .get_or_init(|| DisplayIndex::read(file, self.display))
        {
            Ok(index) => index.get(file, id),
            Err(err) => Err(anyhow::anyhow!("{err:#}")),
        }
    }
}

fn decode_meta(compressed: &[u8]) -> anyhow::Result<MetaSection> {
    let raw = zstd::decode_all(compressed)?;
    // `bitcode::deserialize` rejects trailing bytes, preserving the format's
    // strict section-boundary validation.
    Ok(bitcode::deserialize(raw.as_slice())?)
}

fn read_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Serialize geo data into the sectioned format. Border blob indices are
/// assigned in tile-index order; the reader reconstructs the same order.
pub fn write_geo_data(
    entities: &[GeoEntity],
    worldview_id: &str,
    tile_lookup: &[TileMembership],
    block_lookup: &BTreeMap<(u16, u16), Vec<u32>>,
    provenance_hash: [u8; 32],
) -> anyhow::Result<Vec<u8>> {
    write_geo_data_planes(
        entities,
        worldview_id,
        &[PlaneRaster {
            plane: BitmapPlane::Mercator,
            tiles: tile_lookup,
            blocks: block_lookup,
        }],
        provenance_hash,
    )
}

pub struct PlaneRaster<'a> {
    pub plane: BitmapPlane,
    pub tiles: &'a [TileMembership],
    pub blocks: &'a BTreeMap<(u16, u16), Vec<u32>>,
}

/// Write raster coverage without a display section.
pub fn write_geo_data_planes(
    entities: &[GeoEntity],
    worldview_id: &str,
    rasters: &[PlaneRaster<'_>],
    provenance_hash: [u8; 32],
) -> anyhow::Result<Vec<u8>> {
    write_sections(entities, worldview_id, rasters, provenance_hash, &[])
}

/// Write raster coverage and display geometry in a single worldview asset.
/// An empty record list still writes a display section.
pub fn write_geo_data_planes_with_display(
    entities: &[GeoEntity],
    worldview_id: &str,
    rasters: &[PlaneRaster<'_>],
    provenance_hash: [u8; 32],
    records: &[DisplayRecord],
) -> anyhow::Result<Vec<u8>> {
    let display = write_display_section(worldview_id, records)?;
    write_sections(entities, worldview_id, rasters, provenance_hash, &display)
}

fn write_sections(
    entities: &[GeoEntity],
    worldview_id: &str,
    rasters: &[PlaneRaster<'_>],
    provenance_hash: [u8; 32],
    display: &[u8],
) -> anyhow::Result<Vec<u8>> {
    for (i, raster) in rasters.iter().enumerate() {
        let width = raster.plane.grid_width() as usize;
        anyhow::ensure!(
            raster.tiles.len() == width * width,
            "wrong tile count for {:?}",
            raster.plane
        );
        anyhow::ensure!(
            !rasters[..i].iter().any(|r| r.plane == raster.plane),
            "duplicate plane {:?}",
            raster.plane
        );
    }
    let mut tile_index_raw = Vec::with_capacity(ALL_PLANE_TILE_COUNT * 5);
    let mut blobs: Vec<Vec<u8>> = Vec::new();
    for plane in BitmapPlane::ALL {
        let width = plane.grid_width() as usize;
        let raster = rasters.iter().find(|r| r.plane == plane);
        for idx in 0..width * width {
            let m = raster.map_or(&TileMembership::None, |r| &r.tiles[idx]);
            let (tag, payload): (u8, u32) = match m {
                TileMembership::None => (0, 0),
                TileMembership::Single(id) => {
                    check_tile_payload("entity id", id.0)?;
                    (1, id.0)
                }
                TileMembership::Border => {
                    let (tx, ty) = ((idx / width) as u16, (idx % width) as u16);
                    let cells = raster
                        .and_then(|r| r.blocks.get(&(tx, ty)))
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "border tile {plane:?} ({tx},{ty}) missing block_lookup entry"
                            )
                        })?;
                    let cells: Vec<Option<GeoEntityId>> = cells
                        .iter()
                        .map(|&v| (v != crate::NO_ENTITY).then_some(GeoEntityId(v)))
                        .collect();
                    let blob = PackedTile::try_from_dense(&cells)
                        .map_err(|e| e.context(format!("border tile ({tx},{ty})")))?
                        .to_compressed_bytes();
                    let blob_idx = blobs.len() as u32;
                    check_tile_payload("border blob index", blob_idx)?;
                    blobs.push(blob);
                    (3, blob_idx)
                }
            };
            tile_index_raw.push(tag);
            tile_index_raw.extend_from_slice(&payload.to_le_bytes());
        }
    }

    let meta = MetaSection {
        entities: entities.to_vec(),
        worldview_id: worldview_id.to_owned(),
    };
    let meta_bytes = zstd::encode_all(bitcode::serialize(&meta)?.as_slice(), META_ZSTD_LEVEL)?;
    let tile_index_bytes = zstd::encode_all(tile_index_raw.as_slice(), TILE_INDEX_ZSTD_LEVEL)?;

    let mut border_offsets = Vec::with_capacity(blobs.len() * 8);
    let mut border_blobs = Vec::new();
    for blob in &blobs {
        let off = u32::try_from(border_blobs.len())?;
        let len = u32::try_from(blob.len())?;
        border_offsets.extend_from_slice(&off.to_le_bytes());
        border_offsets.extend_from_slice(&len.to_le_bytes());
        border_blobs.extend_from_slice(blob);
    }

    let sections = [
        &meta_bytes,
        &tile_index_bytes,
        &border_offsets,
        &border_blobs,
        display,
    ];
    let mut offset = HEADER_LEN as u32;
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&provenance_hash);
    for section in sections {
        let len = u32::try_from(section.len())?;
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        offset = offset
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("geo_data: file too large"))?;
    }
    out.reserve(offset as usize - HEADER_LEN);
    for section in sections {
        out.extend_from_slice(section);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub provenance_hash: [u8; 32],
    pub meta: Section,
    pub tile_index: Section,
    pub border_offsets: Section,
    pub border_blobs: Section,
    pub display: Section,
}

fn parse_header(header: &[u8]) -> anyhow::Result<(Header, usize)> {
    anyhow::ensure!(
        header.len() >= HEADER_LEN,
        "geo_data: too short ({} bytes)",
        header.len()
    );
    anyhow::ensure!(
        &header[0..crate::PROVENANCE_HASH_OFFSET] == MAGIC,
        "geo_data: bad magic"
    );
    let mut provenance_hash = [0u8; 32];
    provenance_hash
        .copy_from_slice(&header[crate::PROVENANCE_HASH_OFFSET..crate::PROVENANCE_HASH_END]);
    let sec = |i: usize| -> Section {
        let base = crate::PROVENANCE_HASH_END + i * 8;
        Section {
            off: read_u32(header, base) as usize,
            len: read_u32(header, base + 4) as usize,
        }
    };
    let parsed = Header {
        provenance_hash,
        meta: sec(0),
        tile_index: sec(1),
        border_offsets: sec(2),
        border_blobs: sec(3),
        display: sec(4),
    };
    // The first section must start at the end of this header. Older MGEO
    // headers had four spans and placed it earlier, so they cannot pass.
    let mut end = HEADER_LEN as u32;
    for (name, section) in [
        ("meta", parsed.meta),
        ("tile index", parsed.tile_index),
        ("border offsets", parsed.border_offsets),
        ("border blobs", parsed.border_blobs),
        ("display", parsed.display),
    ] {
        anyhow::ensure!(
            section.off == end as usize,
            "geo_data: invalid {name} offset {} (expected {end})",
            section.off
        );
        end = end
            .checked_add(section.len as u32)
            .ok_or_else(|| anyhow::anyhow!("geo_data: {name} span overflows"))?;
    }
    Ok((parsed, end as usize))
}

impl GeoData {
    pub fn open(path: &Path) -> anyhow::Result<GeoData> {
        Self::open_inner(path, false)
    }

    /// Open an installed file even if only its display tail was truncated.
    pub fn open_for_coverage(path: &Path) -> anyhow::Result<GeoData> {
        Self::open_inner(path, true)
    }

    fn open_inner(path: &Path, allow_truncated_display: bool) -> anyhow::Result<GeoData> {
        let file = File::open(path)
            .map_err(|e| anyhow::anyhow!("geo_data: opening {}: {e}", path.display()))?;
        let mut header_bytes = [0u8; HEADER_LEN];
        read_exact_at(&file, &mut header_bytes, 0)
            .map_err(|e| anyhow::anyhow!("geo_data: reading header: {e}"))?;
        let (header, expected) = parse_header(&header_bytes)?;
        let actual = file.metadata()?.len();
        anyhow::ensure!(
            actual == expected as u64
                || (allow_truncated_display
                    && actual >= header.display.off as u64
                    && actual < expected as u64),
            "geo_data: file is {actual} bytes, header says {expected}"
        );
        let read = |s: Section| -> anyhow::Result<Vec<u8>> {
            let mut buf = vec![0u8; s.len];
            read_exact_at(&file, &mut buf, s.off as u64)
                .map_err(|e| anyhow::anyhow!("geo_data: reading section: {e}"))?;
            Ok(buf)
        };

        let meta_compressed = read(header.meta)?.into_boxed_slice();
        let worldview_id = decode_meta(&meta_compressed)?.worldview_id;

        let tile_raw = zstd::decode_all(read(header.tile_index)?.as_slice())?;
        anyhow::ensure!(
            tile_raw.len() == ALL_PLANE_TILE_COUNT * 5,
            "geo_data: tile index size {} != {}",
            tile_raw.len(),
            ALL_PLANE_TILE_COUNT * 5
        );
        let tile_index = TileIndex::from_raw(&tile_raw)?;

        anyhow::ensure!(
            header.border_offsets.len.is_multiple_of(8),
            "geo_data: border offset table misaligned"
        );
        let border_blobs = BorderBlobs {
            file,
            offsets: header.border_offsets,
            region: header.border_blobs,
        };
        if let Some(max) = tile_index.max_border_index() {
            let count = border_blobs.count();
            anyhow::ensure!(
                (max as usize) < count,
                "geo_data: Border index {max} out of range ({count} blobs)"
            );
        }

        Ok(GeoData {
            worldview_id,
            tile_index,
            border_blobs,
            provenance_hash: header.provenance_hash,
            meta_compressed,
            display: header.display,
            display_index: OnceLock::new(),
        })
    }
}

/// Read the header only when its section layout and the file length agree.
pub fn read_header(bin_path: &Path) -> anyhow::Result<Option<Header>> {
    let f = match File::open(bin_path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => anyhow::bail!("geo_data: opening {}: {e}", bin_path.display()),
    };
    let file_len = f
        .metadata()
        .map_err(|e| anyhow::anyhow!("geo_data: stat {}: {e}", bin_path.display()))?
        .len();
    let mut buf = [0u8; HEADER_LEN];
    match read_exact_at(&f, &mut buf, 0) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    Ok(match parse_header(&buf) {
        Ok((header, expected)) if expected as u64 == file_len => Some(header),
        _ => None,
    })
}
