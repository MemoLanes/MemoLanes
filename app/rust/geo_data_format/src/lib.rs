//! On-disk binary format for MemoLanes geo reference data.
//! Shared by the runtime (`memolanes_core`) and the offline rasterizer
//! (`tools/geo_rasterizer/`).

pub const MAGIC: &[u8; 4] = b"MGEO";

/// Version of the geo-data semantics: the on-disk layout *and* the
/// rasterization algorithm that produces it. **Bump this whenever a
/// change would make an existing `geo_data.bin` stale even though its
/// source inputs are unchanged** — a format-layout change, or any
/// rasterizer change that alters cell/tile semantics.
///
/// It is folded into the provenance hash (see the rasterizer's
/// `compute_provenance_hash`), so bumping it makes the rasterizer's
/// smart-skip rebuild and invalidates any runtime consumer cache
/// without relying on a manual "delete the .bin" step.
pub const GEO_DATA_VERSION: u32 = 1;

/// Byte offset of the 32-byte provenance hash within the file header
/// (immediately after the 4-byte `MAGIC`). The section table follows
/// the hash. Keep readers in sync with this — see `format.rs`.
pub const PROVENANCE_HASH_OFFSET: usize = 4; // MAGIC.len()
/// Length of the provenance hash (SHA-256).
pub const PROVENANCE_HASH_LEN: usize = 32;
/// One-past-the-end of the provenance hash; also the minimum bytes
/// needed to read the hash from an existing file.
pub const PROVENANCE_HASH_END: usize = PROVENANCE_HASH_OFFSET + PROVENANCE_HASH_LEN;

/// Block-grid side length within one tile (128×128 cells per tile).
pub const TILE_WIDTH: usize = 128;
/// Cells per tile = 128 × 128.
pub const CELLS_PER_TILE: usize = TILE_WIDTH * TILE_WIDTH;
/// Tile-grid side length (512×512 tiles).
pub const TILE_GRID_WIDTH: usize = 512;
/// Total tiles in the tile grid.
pub const TILE_COUNT: usize = TILE_GRID_WIDTH * TILE_GRID_WIDTH;

/// x-major tile index in the tile grid: `tx * TILE_GRID_WIDTH + ty`. The one
/// definition of tile ordering, shared by the rasterizer, writer, and runtime
/// reader — matching `BlockKey::index()`'s x-major convention at every grain.
pub fn tile_index(tx: u16, ty: u16) -> usize {
    tx as usize * TILE_GRID_WIDTH + ty as usize
}

/// Inverse of [`tile_index`]: `(tx, ty)` from a tile-grid index.
pub fn tile_xy(idx: usize) -> (u16, u16) {
    (
        (idx / TILE_GRID_WIDTH) as u16,
        (idx % TILE_GRID_WIDTH) as u16,
    )
}

/// x-major cell index within a tile: `x * TILE_WIDTH + y`. The index a
/// [`PackedTile`] lookup expects, equal to `BlockKey::index()`.
pub fn cell_index(x: u8, y: u8) -> usize {
    x as usize * TILE_WIDTH + y as usize
}

mod format;
mod packed_tile;
mod pov;
mod types;

pub use format::{
    expected_total_len, read_geo_data, write_geo_data, GeoData, TileEntry, HEADER_LEN,
};
pub use packed_tile::PackedTile;
pub use pov::*;
pub use types::*;
