//! Eager storage for sparse journey coverage.
use super::{BlockKey, Tile, TileKey};
use anyhow::Result;
use std::borrow::Cow;
use std::collections::{hash_map::Entry, HashMap};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TileStorage {
    tiles: HashMap<TileKey, Tile>,
}

impl TileStorage {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        // A tile has a large inline block table, so bound eager preallocation.
        Self {
            tiles: HashMap::with_capacity(capacity.min(4)),
        }
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = &TileKey> {
        self.tiles.keys()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
    pub(super) fn len(&self) -> usize {
        self.tiles.len()
    }
    pub(super) fn contains_key(&self, key: &TileKey) -> bool {
        self.tiles.contains_key(key)
    }
    pub(super) fn peek_tile<F, R>(&self, key: &TileKey, mut f: F) -> R
    where
        F: FnMut(Option<&Tile>) -> R,
    {
        f(self.tiles.get(key))
    }
    pub(super) fn get_tile(&mut self, key: &TileKey) -> Option<&Tile> {
        self.tiles.get(key)
    }
    pub(super) fn get_tile_mut(&mut self, key: &TileKey) -> Option<&mut Tile> {
        self.tiles.get_mut(key)
    }
    pub(super) fn get_tile_summary(&mut self, key: &TileKey) -> Option<TileSummary<'_>> {
        self.tiles.get(key).map(TileSummary)
    }
    pub(super) fn get_tile_bytes(&mut self, key: &TileKey) -> Option<Cow<'_, [u8]>> {
        self.tiles.get(key).map(|tile| Cow::Owned(tile.serialize()))
    }
    pub(super) fn mutate_or_insert_empty<R, F>(&mut self, key: &TileKey, f: F) -> (R, bool)
    where
        F: FnOnce(&mut Tile) -> (R, bool),
    {
        let (tile, inserted) = match self.tiles.entry(*key) {
            Entry::Occupied(entry) => (entry.into_mut(), false),
            Entry::Vacant(entry) => (entry.insert(Tile::new()), true),
        };
        let (result, changed) = f(tile);
        if inserted && !changed {
            self.tiles.remove(key);
        }
        (result, changed)
    }
    pub(super) fn insert_tile(&mut self, key: TileKey, tile: Tile) {
        self.tiles.insert(key, tile);
    }
    pub(super) fn insert_bytes(&mut self, key: TileKey, bytes: Vec<u8>) -> Result<()> {
        let tile = Tile::deserialize(&bytes)?;
        self.tiles.insert(key, tile);
        Ok(())
    }
    pub(super) fn insert_entry(&mut self, key: TileKey, tile: Tile) {
        self.tiles.insert(key, tile);
    }
    pub(super) fn into_entries(self) -> impl Iterator<Item = (TileKey, Tile)> {
        self.tiles.into_iter()
    }
    pub(super) fn clone_entry(&self, key: &TileKey) -> Tile {
        self.tiles[key].clone()
    }
    pub(super) fn decode_entry(tile: Tile) -> Tile {
        tile
    }
    pub(super) fn remove(&mut self, key: &TileKey) {
        self.tiles.remove(key);
    }
    pub(super) fn validate_tile(&mut self, key: &TileKey) -> Result<bool> {
        let tile = self.tiles.get_mut(key).unwrap();
        let changed = tile.remove_empty_blocks();
        let empty = tile.is_empty();
        if empty {
            self.tiles.remove(key);
        }
        Ok(changed || empty)
    }
    pub(super) fn check_invariant_and_debug_log(&mut self) {
        assert!(self.tiles.keys().all(|key| key.is_in_bounds()));
    }
}

pub(super) struct TileSummary<'a>(&'a Tile);
impl TileSummary<'_> {
    pub(super) fn contains_block(&self, key: &BlockKey) -> bool {
        self.0.get(key).is_some()
    }
    pub(super) fn iter_blocks(&self) -> impl Iterator<Item = BlockKey> + '_ {
        self.0.iter().map(|(key, _)| key)
    }
}
