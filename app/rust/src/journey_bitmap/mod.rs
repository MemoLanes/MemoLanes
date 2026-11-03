//! Sparse journey coverage and scoped editing across three projection grids.
//!
//! Storage owns tile encoding and decoding. Coordinates, editing and
//! rasterization operate independently of the stored representation.

pub mod area;
mod bounds;
mod editor;
pub(crate) mod geometry;
mod serialization;
mod storage;
mod tile;

pub use bounds::get_bounds_from_journey_bitmap;
pub use editor::BitmapEditor;
pub use geometry::{
    BitmapPlane, BITMAP_WIDTH, BITMAP_WIDTH_OFFSET, MAP_WIDTH, MAP_WIDTH_OFFSET,
    MERCATOR_MAX_LATITUDE, POLAR_MAP_WIDTH, POLAR_MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET,
};
pub use serialization::{deserialize_journey_bitmap, serialize_journey_bitmap};
pub use tile::{Block, BlockKey, Tile, BITMAP_SIZE, MIPMAP_BIT_SIZE};

use anyhow::Result;
use std::borrow::Cow;
use storage::TileStorage;

/// A tile's complete identity: projection and coordinates local to that grid.
/// Match the projection variant to access coordinates in the correct grid.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum TileKey {
    Mercator { x: u16, y: u16 },
    NorthPolar { x: u16, y: u16 },
    SouthPolar { x: u16, y: u16 },
}

impl TileKey {
    pub fn new(plane: BitmapPlane, x: u16, y: u16) -> Self {
        match plane {
            BitmapPlane::Mercator => Self::Mercator { x, y },
            BitmapPlane::NorthPolar => Self::NorthPolar { x, y },
            BitmapPlane::SouthPolar => Self::SouthPolar { x, y },
        }
    }

    pub const fn mercator(x: u16, y: u16) -> Self {
        Self::Mercator { x, y }
    }

    pub fn plane(self) -> BitmapPlane {
        match self {
            Self::Mercator { .. } => BitmapPlane::Mercator,
            Self::NorthPolar { .. } => BitmapPlane::NorthPolar,
            Self::SouthPolar { .. } => BitmapPlane::SouthPolar,
        }
    }

    fn is_in_bounds(self) -> bool {
        match self {
            Self::Mercator { x, y } => x < MAP_WIDTH as u16 && y < MAP_WIDTH as u16,
            Self::NorthPolar { x, y } | Self::SouthPolar { x, y } => {
                x < POLAR_MAP_WIDTH as u16 && y < POLAR_MAP_WIDTH as u16
            }
        }
    }
}

/// Lightweight block occupancy, independent of the storage representation.
pub struct TileSummary<'a>(storage::TileSummary<'a>);

impl TileSummary<'_> {
    pub fn contains_block(&self, block_key: &BlockKey) -> bool {
        self.0.contains_block(block_key)
    }

    pub(crate) fn iter_blocks(&self) -> impl Iterator<Item = BlockKey> + '_ {
        self.0.iter_blocks()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JourneyBitmap {
    storage: TileStorage,
}

impl JourneyBitmap {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_tile_capacity(capacity: usize) -> Self {
        Self {
            storage: TileStorage::with_capacity(capacity),
        }
    }

    pub fn has_polar_data(&self) -> bool {
        self.all_tile_keys()
            .any(|key| key.plane() != BitmapPlane::Mercator)
    }

    pub fn has_plane_data(&self, plane: BitmapPlane) -> bool {
        self.all_plane_tile_keys(plane).next().is_some()
    }

    /// Load imported tile records. Call `edit().validate()` before using
    /// untrusted input; eager storage may report decoding errors here already.
    pub fn of_tile_bytes_without_validation(data: Vec<(TileKey, Vec<u8>)>) -> Result<Self> {
        let mut bitmap = Self::new();
        let mut editor = bitmap.edit();
        for (key, bytes) in data {
            editor.insert_tile_bytes(key, bytes)?;
        }
        Ok(bitmap)
    }

    /// Includes all projections. Mercator-only consumers must match explicitly.
    pub fn all_tile_keys(&self) -> impl Iterator<Item = &TileKey> {
        self.storage.keys()
    }

    pub fn all_plane_tile_keys(&self, plane: BitmapPlane) -> impl Iterator<Item = &TileKey> {
        self.all_tile_keys().filter(move |key| key.plane() == plane)
    }

    pub fn peek_tile_without_updating_cache<F, R>(&self, key: &TileKey, f: F) -> R
    where
        F: FnMut(Option<&Tile>) -> R,
    {
        self.storage.peek_tile(key, f)
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub fn tile_count(&self) -> usize {
        self.storage.len()
    }

    pub fn contains_tile(&self, key: &TileKey) -> bool {
        self.storage.contains_key(key)
    }

    pub fn get_tile(&mut self, key: &TileKey) -> Option<&Tile> {
        self.storage.get_tile(key)
    }

    pub fn get_tile_summary(&mut self, key: &TileKey) -> Option<TileSummary<'_>> {
        self.storage.get_tile_summary(key).map(TileSummary)
    }

    /// Cached storage can lend its bytes; eager storage can encode on demand.
    pub fn get_tile_bytes(&mut self, key: &TileKey) -> Option<Cow<'_, [u8]>> {
        self.storage.get_tile_bytes(key)
    }

    pub fn check_invariant_and_debug_log(&mut self) {
        self.storage.check_invariant_and_debug_log();
    }

    /// Edit bitmap content without collecting changed tile keys.
    pub fn edit(&mut self) -> BitmapEditor<'_> {
        BitmapEditor::new(self, false)
    }

    /// Edit bitmap content and collect source tile keys for cache invalidation.
    pub fn edit_tracked(&mut self) -> BitmapEditor<'_> {
        BitmapEditor::new(self, true)
    }
}
