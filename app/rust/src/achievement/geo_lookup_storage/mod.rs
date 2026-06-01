//! Border-tile store. Call sites use the `BorderStore` alias below.

use geo_data_format::GeoEntityId;

mod plain;

/// The lookup surface for the border-tile store.
pub(crate) trait BorderTileLookup {
    fn from_compressed(blobs: Vec<Box<[u8]>>) -> Self
    where
        Self: Sized;
    fn build(tiles: Vec<Vec<Option<GeoEntityId>>>) -> Self
    where
        Self: Sized;
    fn lookup(&self, id: u32, cell_idx: usize) -> Option<GeoEntityId>;
    fn resident_heap_bytes(&self) -> usize;
}

pub(crate) type BorderStore = plain::PlainBorderStore;

#[cfg(test)]
mod shared_tests {
    use super::*;
    use geo_data_format::{GeoEntityId, PackedTile, CELLS_PER_TILE};

    fn make_cells(value: u32) -> Vec<Option<GeoEntityId>> {
        let mut cells = vec![None; CELLS_PER_TILE];
        for cell in cells.iter_mut().take(CELLS_PER_TILE / 2) {
            *cell = Some(GeoEntityId(value));
        }
        cells
    }

    #[test]
    fn lookup_returns_same_values_as_dense() {
        let dense_a = make_cells(1);
        let dense_b = make_cells(2);
        let store = BorderStore::build(vec![dense_a.clone(), dense_b.clone()]);
        for (i, expected) in dense_a.iter().enumerate() {
            assert_eq!(store.lookup(0, i), *expected);
        }
        for (i, expected) in dense_b.iter().enumerate() {
            assert_eq!(store.lookup(1, i), *expected);
        }
    }

    #[test]
    fn from_compressed_round_trips() {
        let dense = make_cells(42);
        let blob = PackedTile::from_dense(&dense).to_compressed_bytes();
        let store = BorderStore::from_compressed(vec![blob.into_boxed_slice()]);
        assert_eq!(store.len(), 1);
        for (i, expected) in dense.iter().enumerate() {
            assert_eq!(store.lookup(0, i), *expected);
        }
    }
}
