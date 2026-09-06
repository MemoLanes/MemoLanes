//! Sectioned on-disk geo data format.
//!
//! Layout: `Header(68 B) | Meta | TileIndex | BorderOffsets | BorderBlobs`.
//! All integers little-endian. Border tiles are stored already
//! `PackedTile`-compressed. [`GeoData::open`] keeps the tile index (sparse)
//! and the compressed meta section in memory; border offsets and blobs stay
//! on disk and are read per lookup, entities are decoded per request.

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
        let words = TILE_COUNT / TILES_PER_WORD;
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
struct Section {
    off: usize,
    len: usize,
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
    /// The worldview id this asset represents (see [`MetaSection::worldview_id`]).
    pub worldview_id: String,
    pub tile_index: TileIndex,
    pub border_blobs: BorderBlobs,
    pub provenance_hash: [u8; 32],
    meta_compressed: Box<[u8]>,
}

impl GeoData {
    pub fn entities(&self) -> anyhow::Result<Vec<GeoEntity>> {
        Ok(decode_meta(&self.meta_compressed)?.entities)
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
            TileMembership::Single(id) => {
                check_tile_payload("entity id", id.0)?;
                (1, id.0)
            }
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
                check_tile_payload("border blob index", blob_idx)?;
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

        let meta_compressed = read(header.meta)?.into_boxed_slice();
        let worldview_id = decode_meta(&meta_compressed)?.worldview_id;

        let tile_raw = zstd::decode_all(read(header.tile_index)?.as_slice())?;
        anyhow::ensure!(
            tile_raw.len() == TILE_COUNT * 5,
            "geo_data: tile index size {} != {}",
            tile_raw.len(),
            TILE_COUNT * 5
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
        })
    }
}
