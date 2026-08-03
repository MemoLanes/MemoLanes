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
pub const GEO_DATA_VERSION: u32 = 3;

pub const PROVENANCE_HASH_OFFSET: usize = 4; // MAGIC.len()
pub const PROVENANCE_HASH_LEN: usize = 32;
pub const PROVENANCE_HASH_END: usize = PROVENANCE_HASH_OFFSET + PROVENANCE_HASH_LEN;

/// Cell value meaning "no entity owns this cell" in a dense border-tile
/// buffer. Use this rather than `Option` to save 4 bytes per entity
/// during rasterization
pub const NO_ENTITY: u32 = u32::MAX;

pub const TILE_WIDTH: usize = 128;
pub const CELLS_PER_TILE: usize = TILE_WIDTH * TILE_WIDTH;
pub const TILE_GRID_WIDTH: usize = 512;
pub const TILE_COUNT: usize = TILE_GRID_WIDTH * TILE_GRID_WIDTH;

/// x-major tile index in the tile grid: `tx * TILE_GRID_WIDTH + ty`. The one
/// definition of tile ordering, shared by the rasterizer, writer, and runtime
/// reader — matching `BlockKey::index()`'s x-major convention at every grain.
pub fn tile_index(tx: u16, ty: u16) -> usize {
    tx as usize * TILE_GRID_WIDTH + ty as usize
}

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

mod cldr;
mod format;
mod locale;
mod packed_tile;
mod types;
mod worldview;

pub use cldr::*;
pub use format::{
    expected_total_len, read_geo_data, write_geo_data, GeoData, TileEntry, HEADER_LEN,
};
pub use locale::*;
pub use packed_tile::PackedTile;
pub use types::*;
pub use worldview::*;
