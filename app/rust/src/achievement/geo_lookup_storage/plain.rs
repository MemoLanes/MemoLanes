//! Border-tile store: expands each on-disk blob into a dense per-cell
//! array at load and indexes into it directly.

use geo_data_format::{GeoEntityId, PackedTile, CELLS_PER_TILE};

use super::BorderTileLookup;

#[doc(hidden)]
pub struct PlainBorderStore {
    tiles: Vec<Box<[Option<GeoEntityId>]>>,
}

impl BorderTileLookup for PlainBorderStore {
    fn build(tiles: Vec<Vec<Option<GeoEntityId>>>) -> Self {
        Self {
            tiles: tiles
                .into_iter()
                .map(|cells| {
                    debug_assert_eq!(
                        cells.len(),
                        CELLS_PER_TILE,
                        "PlainBorderStore::build: tile must have CELLS_PER_TILE cells"
                    );
                    cells.into_boxed_slice()
                })
                .collect(),
        }
    }

    fn from_compressed(blobs: Vec<Box<[u8]>>) -> Self {
        let tiles = blobs
            .into_iter()
            .map(|blob| {
                PackedTile::from_compressed_bytes(&blob)
                    .to_dense()
                    .into_boxed_slice()
            })
            .collect();
        Self { tiles }
    }

    fn lookup(&self, id: u32, cell_idx: usize) -> Option<GeoEntityId> {
        self.tiles[id as usize][cell_idx]
    }

    fn resident_heap_bytes(&self) -> usize {
        self.tiles.len() * CELLS_PER_TILE * std::mem::size_of::<Option<GeoEntityId>>()
    }
}

#[cfg(test)]
impl PlainBorderStore {
    pub(crate) fn len(&self) -> usize {
        self.tiles.len()
    }
}

#[cfg(test)]
mod plain_tests {
    use super::*;

    fn make_cells(value: u32) -> Vec<Option<GeoEntityId>> {
        let mut cells = vec![None; CELLS_PER_TILE];
        for cell in cells.iter_mut().take(CELLS_PER_TILE / 2) {
            *cell = Some(GeoEntityId(value));
        }
        cells
    }

    #[test]
    fn build_then_lookup_matches_dense() {
        let dense = make_cells(5);
        let store = PlainBorderStore::build(vec![dense.clone()]);
        assert_eq!(store.len(), 1);
        for (i, expected) in dense.iter().enumerate() {
            assert_eq!(store.lookup(0, i), *expected);
        }
    }

    #[test]
    fn from_compressed_matches_dense() {
        let dense = make_cells(9);
        let blob = PackedTile::from_dense(&dense)
            .to_compressed_bytes()
            .into_boxed_slice();
        let store = PlainBorderStore::from_compressed(vec![blob]);
        for (i, expected) in dense.iter().enumerate() {
            assert_eq!(store.lookup(0, i), *expected);
        }
    }

    #[test]
    fn multi_tile_lookup_indexes_by_id() {
        let t0 = make_cells(1);
        let t1 = make_cells(2);
        let store = PlainBorderStore::build(vec![t0.clone(), t1.clone()]);
        assert_eq!(store.len(), 2);
        for (i, expected) in t0.iter().enumerate() {
            assert_eq!(store.lookup(0, i), *expected);
        }
        for (i, expected) in t1.iter().enumerate() {
            assert_eq!(store.lookup(1, i), *expected);
        }
    }
}
