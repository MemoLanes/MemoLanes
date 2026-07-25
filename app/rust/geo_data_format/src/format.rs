//! Sectioned on-disk geo data format.
//!
//! Layout: `Header(68 B) | Meta | TileIndex | BorderOffsets | BorderBlobs`.
//! All integers little-endian. Border tiles are stored already
//! `PackedTile`-compressed so the runtime loads them by slice-copy with
//! no dense intermediate. See the design spec.

use std::collections::BTreeMap;

use bincode::Options;
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

/// Fully parsed geo data. `border_blobs[i]` is the still-compressed
/// `PackedTile` for the tile whose `TileEntry::Border(i)` references it.
#[derive(Debug)]
pub struct GeoData {
    pub entities: Vec<GeoEntity>,
    /// The worldview id this asset represents (see [`MetaSection::worldview_id`]).
    pub worldview_id: String,
    pub tile_index: Vec<TileEntry>,
    pub border_blobs: Vec<Box<[u8]>>,
    pub provenance_hash: [u8; 32],
}

fn bincode_opts() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
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
    block_lookup: &BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>>,
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
                let blob = PackedTile::try_from_dense(cells)
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
    let meta_bytes =
        zstd::encode_all(bincode_opts().serialize(&meta)?.as_slice(), META_ZSTD_LEVEL)?;
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

/// Parse the sectioned format. No dense border tile is ever materialized:
/// each blob is slice-copied out still-compressed.
pub fn read_geo_data(bytes: &[u8]) -> anyhow::Result<GeoData> {
    anyhow::ensure!(
        bytes.len() >= HEADER_LEN,
        "geo_data: too short ({} bytes)",
        bytes.len()
    );
    anyhow::ensure!(
        &bytes[0..crate::PROVENANCE_HASH_OFFSET] == MAGIC,
        "geo_data: bad magic"
    );
    let mut provenance_hash = [0u8; 32];
    provenance_hash
        .copy_from_slice(&bytes[crate::PROVENANCE_HASH_OFFSET..crate::PROVENANCE_HASH_END]);

    let sec = |i: usize| -> (usize, usize) {
        let base = crate::PROVENANCE_HASH_END + i * 8;
        (
            read_u32(bytes, base) as usize,
            read_u32(bytes, base + 4) as usize,
        )
    };
    let (meta_off, meta_len) = sec(0);
    let (tile_off, tile_len) = sec(1);
    let (boff_off, boff_len) = sec(2);
    let (blob_off, blob_len) = sec(3);

    let slice = |off: usize, len: usize| -> anyhow::Result<&[u8]> {
        bytes
            .get(off..off + len)
            .ok_or_else(|| anyhow::anyhow!("geo_data: section out of bounds"))
    };

    let meta_raw = zstd::decode_all(slice(meta_off, meta_len)?)?;
    let meta: MetaSection = bincode_opts().deserialize(meta_raw.as_slice())?;

    let tile_raw = zstd::decode_all(slice(tile_off, tile_len)?)?;
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

    let boff = slice(boff_off, boff_len)?;
    anyhow::ensure!(
        boff_len % 8 == 0,
        "geo_data: border offset table misaligned"
    );
    let blob_region = slice(blob_off, blob_len)?;
    let n = boff_len / 8;
    let mut border_blobs = Vec::with_capacity(n);
    for i in 0..n {
        let off = read_u32(boff, i * 8) as usize;
        let len = read_u32(boff, i * 8 + 4) as usize;
        let b = blob_region
            .get(off..off + len)
            .ok_or_else(|| anyhow::anyhow!("geo_data: blob {i} out of bounds"))?;
        border_blobs.push(b.to_vec().into_boxed_slice());
    }

    let blob_count = border_blobs.len();
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
        border_blobs,
        provenance_hash,
    })
}
