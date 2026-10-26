use bitmap_geometry::{BitmapPlane, POLAR_MAP_WIDTH};

pub const MAGIC: &[u8; 4] = b"MGEO";

/// Version of the geo-data semantics: the on-disk layout *and* the
/// rasterization algorithm that produces it. **Bump this whenever a
/// change would make an existing `geo_data.bin` stale even though its
/// source inputs are unchanged** — a format-layout change, any
/// rasterizer change that alters cell/tile semantics, or an editorial
/// policy-logic change with no `geo_policy.toml` row (e.g. the
/// coextensive sole-unit dedup rule).
///
/// It is folded into the provenance hash (see the rasterizer's
/// `compute_provenance_hash`), so bumping it makes the rasterizer's
/// smart-skip rebuild and invalidates any runtime consumer cache
/// without relying on a manual "delete the .bin" step.
pub const GEO_DATA_VERSION: u32 = 5;

pub const PROVENANCE_HASH_OFFSET: usize = 4; // MAGIC.len()
pub const PROVENANCE_HASH_LEN: usize = 32;
pub const PROVENANCE_HASH_END: usize = PROVENANCE_HASH_OFFSET + PROVENANCE_HASH_LEN;

/// Cell value meaning "no entity owns this cell" in a dense border-tile
/// buffer. Use this rather than `Option` to save 4 bytes per entity
/// during rasterization
pub const NO_ENTITY: u32 = u32::MAX;

pub const TILE_WIDTH: usize = bitmap_geometry::TILE_WIDTH as usize;
pub const CELLS_PER_TILE: usize = TILE_WIDTH * TILE_WIDTH;
pub const TILE_GRID_WIDTH: usize = bitmap_geometry::MAP_WIDTH as usize;
pub const TILE_COUNT: usize = TILE_GRID_WIDTH * TILE_GRID_WIDTH;
pub const POLAR_TILE_COUNT: usize = (POLAR_MAP_WIDTH * POLAR_MAP_WIDTH) as usize;
pub const ALL_PLANE_TILE_COUNT: usize = TILE_COUNT + 2 * POLAR_TILE_COUNT;

pub fn plane_tile_index(plane: BitmapPlane, tx: u16, ty: u16) -> Option<usize> {
    let width = plane.grid_width() as usize;
    if tx as usize >= width || ty as usize >= width {
        return None;
    }
    let offset = match plane {
        BitmapPlane::Mercator => 0,
        BitmapPlane::NorthPolar => TILE_COUNT,
        BitmapPlane::SouthPolar => TILE_COUNT + POLAR_TILE_COUNT,
    };
    Some(offset + tx as usize * width + ty as usize)
}

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
    expected_total_len, read_header, read_provenance_hash, write_geo_data, write_geo_data_planes,
    BorderBlobs, GeoData, Header, PlaneRaster, Section, TileEntry, TileIndex, HEADER_LEN,
};
pub use locale::*;
pub use packed_tile::PackedTile;
pub use types::*;
pub use worldview::*;
