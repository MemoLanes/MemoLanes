//! Sectioned on-disk geo data format.
//!
//! Layout: `Header(68 B) | Meta | TileIndex | BorderOffsets | BorderBlobs`.
//! All integers little-endian. Border tiles are stored already
//! `PackedTile`-compressed, and the blob region is the one section a reader
//! may leave on disk: [`GeoData::open`] keeps only its span table in memory
//! and reads each blob on demand. See the design spec.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    tile_xy, GeoEntity, GeoEntityId, PackedTile, TileMembership, MAGIC, PROVENANCE_HASH_END,
    TILE_COUNT,
};

/// magic(4) + provenance_hash(32) + 4 sections × (u32 offset, u32 len).
pub const HEADER_LEN: usize = PROVENANCE_HASH_END + 4 * 8; // PROVENANCE_HASH_END=36, sections table=32
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileEntry {
    Single(GeoEntityId),
    Border(u32),
    None,
}

/// Border tiles' compressed `PackedTile` blobs, left in the file and read on
/// demand; only the span table is resident.
#[derive(Debug)]
pub struct BorderBlobs {
    spans: Box<[(u32, u32)]>,
    file: File,
    region_offset: u64,
}

impl BorderBlobs {
    pub fn get(&self, i: u32) -> anyhow::Result<Vec<u8>> {
        let (off, len) = *self
            .spans
            .get(i as usize)
            .ok_or_else(|| anyhow::anyhow!("geo_data: no border blob {i}"))?;
        let mut buf = vec![0u8; len as usize];
        read_exact_at(&self.file, &mut buf, self.region_offset + off as u64)
            .map_err(|e| anyhow::anyhow!("geo_data: reading border blob {i}: {e}"))?;
        Ok(buf)
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    while !buf.is_empty() {
        match std::os::windows::fs::FileExt::seek_read(file, buf, offset)? {
            0 => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            n => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct GeoData {
    pub entities: Vec<GeoEntity>,
    /// The worldview id this asset represents (see [`MetaSection::worldview_id`]).
    pub worldview_id: String,
    pub tile_index: Vec<TileEntry>,
    pub border_blobs: BorderBlobs,
    pub provenance_hash: [u8; 32],
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
    anyhow::ensure!(
        tile_lookup.len() == TILE_COUNT,
        "tile_lookup must have {TILE_COUNT} entries, got {}",
        tile_lookup.len()
    );

    let mut tile_index_raw = Vec::with_capacity(TILE_COUNT * 5);
    let mut blobs: Vec<Vec<u8>> = Vec::new();
    for (idx, m) in tile_lookup.iter().enumerate() {
        let (tag, payload): (u8, u32) = match m {
            TileMembership::None => (0, 0),
            TileMembership::Single(id) => (1, id.0),
            TileMembership::Border => {
                let (tx, ty) = tile_xy(idx);
                let cells = block_lookup.get(&(tx, ty)).ok_or_else(|| {
                    anyhow::anyhow!("border tile ({tx},{ty}) missing block_lookup entry")
                })?;
                let cells: Vec<Option<GeoEntityId>> = cells
                    .iter()
                    .map(|&v| (v != crate::NO_ENTITY).then_some(GeoEntityId(v)))
                    .collect();
                let blob = PackedTile::try_from_dense(&cells)
                    .map_err(|e| e.context(format!("border tile ({tx},{ty})")))?
                    .to_compressed_bytes();
                let blob_idx = blobs.len() as u32;
                blobs.push(blob);
                (3, blob_idx)
            }
        };
        tile_index_raw.push(tag);
        tile_index_raw.extend_from_slice(&payload.to_le_bytes());
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
        let off = border_blobs.len() as u32;
        let len = blob.len() as u32;
        border_offsets.extend_from_slice(&off.to_le_bytes());
        border_offsets.extend_from_slice(&len.to_le_bytes());
        border_blobs.extend_from_slice(blob);
    }

    let meta_off = HEADER_LEN as u32;
    let tile_off = meta_off + meta_bytes.len() as u32;
    let boff_off = tile_off + tile_index_bytes.len() as u32;
    let blob_off = boff_off + border_offsets.len() as u32;

    let mut out = Vec::with_capacity(blob_off as usize + border_blobs.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&provenance_hash);
    for (off, len) in [
        (meta_off, meta_bytes.len() as u32),
        (tile_off, tile_index_bytes.len() as u32),
        (boff_off, border_offsets.len() as u32),
        (blob_off, border_blobs.len() as u32),
    ] {
        out.extend_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
    }
    out.extend_from_slice(&meta_bytes);
    out.extend_from_slice(&tile_index_bytes);
    out.extend_from_slice(&border_offsets);
    out.extend_from_slice(&border_blobs);
    Ok(out)
}

/// Total byte length a complete file must have, derived from its header
/// alone: the maximum of every section's `offset + len`. Returns `None` if
/// `header` is shorter than [`HEADER_LEN`] or the magic doesn't match.
///
/// The smart-skip cache uses this to detect a torn/truncated `geo_data.bin`
/// (actual file size != expected) and rebuild instead of trusting the
/// provenance hash, which sits at the front of the header and would survive
/// a write that left the body truncated.
pub fn expected_total_len(header: &[u8]) -> Option<usize> {
    if header.len() < HEADER_LEN || &header[0..crate::PROVENANCE_HASH_OFFSET] != MAGIC {
        return None;
    }
    let mut total = HEADER_LEN;
    for i in 0..4 {
        let base = crate::PROVENANCE_HASH_END + i * 8;
        let off = read_u32(header, base) as usize;
        let len = read_u32(header, base + 4) as usize;
        total = total.max(off + len);
    }
    Some(total)
}

#[derive(Clone, Copy)]
struct Section {
    off: usize,
    len: usize,
}

struct Header {
    provenance_hash: [u8; 32],
    meta: Section,
    tile_index: Section,
    border_offsets: Section,
    border_blobs: Section,
}

fn parse_header(header: &[u8]) -> anyhow::Result<Header> {
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
    Ok(Header {
        provenance_hash,
        meta: sec(0),
        tile_index: sec(1),
        border_offsets: sec(2),
        border_blobs: sec(3),
    })
}

fn assemble(
    header: Header,
    meta_compressed: &[u8],
    tile_index_compressed: &[u8],
    border_offsets: &[u8],
    file: File,
) -> anyhow::Result<GeoData> {
    let meta_raw = zstd::decode_all(meta_compressed)?;
    // `bitcode::deserialize` rejects trailing bytes, preserving the format's
    // strict section-boundary validation.
    let meta: MetaSection = bitcode::deserialize(meta_raw.as_slice())?;

    let tile_raw = zstd::decode_all(tile_index_compressed)?;
    anyhow::ensure!(
        tile_raw.len() == TILE_COUNT * 5,
        "geo_data: tile index size {} != {}",
        tile_raw.len(),
        TILE_COUNT * 5
    );
    let mut tile_index = Vec::with_capacity(TILE_COUNT);
    for i in 0..TILE_COUNT {
        let tag = tile_raw[i * 5];
        let payload = read_u32(&tile_raw, i * 5 + 1);
        tile_index.push(match tag {
            0 => TileEntry::None,
            1 => TileEntry::Single(GeoEntityId(payload)),
            3 => TileEntry::Border(payload),
            t => anyhow::bail!("geo_data: bad tile tag {t}"),
        });
    }

    anyhow::ensure!(
        border_offsets.len().is_multiple_of(8),
        "geo_data: border offset table misaligned"
    );
    let blob_len = header.border_blobs.len;
    let n = border_offsets.len() / 8;
    let mut spans = Vec::with_capacity(n);
    for i in 0..n {
        let off = read_u32(border_offsets, i * 8);
        let len = read_u32(border_offsets, i * 8 + 4);
        anyhow::ensure!(
            off.checked_add(len)
                .is_some_and(|end| end as usize <= blob_len),
            "geo_data: blob {i} out of bounds"
        );
        spans.push((off, len));
    }
    let blob_count = spans.len();
    for entry in &tile_index {
        if let TileEntry::Border(i) = entry {
            anyhow::ensure!(
                (*i as usize) < blob_count,
                "geo_data: Border index {i} out of range ({blob_count} blobs)"
            );
        }
    }

    Ok(GeoData {
        entities: meta.entities,
        worldview_id: meta.worldview_id,
        tile_index,
        border_blobs: BorderBlobs {
            spans: spans.into_boxed_slice(),
            file,
            region_offset: header.border_blobs.off as u64,
        },
        provenance_hash: header.provenance_hash,
    })
}

impl GeoData {
    pub fn open(path: &Path) -> anyhow::Result<GeoData> {
        let file = File::open(path)
            .map_err(|e| anyhow::anyhow!("geo_data: opening {}: {e}", path.display()))?;
        let mut header_bytes = [0u8; HEADER_LEN];
        read_exact_at(&file, &mut header_bytes, 0)
            .map_err(|e| anyhow::anyhow!("geo_data: reading header: {e}"))?;
        let header = parse_header(&header_bytes)?;
        let expected = expected_total_len(&header_bytes).expect("header already validated") as u64;
        let actual = file.metadata()?.len();
        anyhow::ensure!(
            actual == expected,
            "geo_data: file is {actual} bytes, header says {expected}"
        );
        let read = |s: Section| -> anyhow::Result<Vec<u8>> {
            let mut buf = vec![0u8; s.len];
            read_exact_at(&file, &mut buf, s.off as u64)
                .map_err(|e| anyhow::anyhow!("geo_data: reading section: {e}"))?;
            Ok(buf)
        };
        let meta = read(header.meta)?;
        let tile_index = read(header.tile_index)?;
        let border_offsets = read(header.border_offsets)?;
        assemble(header, &meta, &tile_index, &border_offsets, file)
    }
}
