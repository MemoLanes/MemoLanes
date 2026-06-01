use std::collections::HashMap;

use crate::achievement::geo_lookup_storage::{BorderStore, BorderTileLookup};
use crate::journey_bitmap::{MAP_WIDTH, TILE_WIDTH};

use super::geo_entity::{GeoEntity, GeoEntityId, GeoEntityKind, Worldview};

pub use geo_data_format::TileMembership;

/// In-memory tile classification. Mirrors the on-disk
/// `geo_data_format::TileMembership` except `Border` carries a `u32`
/// index into the local `BorderStore`. Constructed only by
/// `GeoLookupTable::load_from_bytes` (and the test-only constructors).
#[derive(Debug, Clone)]
pub(crate) enum MemTileMembership {
    Single(GeoEntityId),
    Border(u32),
    None,
}

/// Hierarchical geo lookup table operating at tile and block granularity.
/// Reuses the JourneyBitmap coordinate space.
pub struct GeoLookupTable {
    /// Tile level: flat array indexed by tile_y * MAP_WIDTH + tile_x.
    /// Border variants carry an id into `border_store`.
    tile_lookup: Vec<MemTileMembership>,

    /// Border-tile store, keyed by border id.
    border_store: BorderStore,

    /// All entities keyed by ID.
    entities: HashMap<GeoEntityId, GeoEntity>,

    worldviews: Vec<Worldview>,

    /// Matches the on-disk header; used for cache-key invalidation.
    provenance_hash: [u8; 32],
}

impl GeoLookupTable {
    // TODO(geo-C): Phase 2 — load the base, then apply the active
    // worldview's delta over it before building the lookup.
    pub fn load_from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        use geo_data_format::TileEntry;

        let gd = geo_data_format::read_geo_data(data)?;

        let tile_lookup: Vec<MemTileMembership> = gd
            .tile_index
            .into_iter()
            .map(|e| match e {
                TileEntry::Single(v) => MemTileMembership::Single(v),
                TileEntry::None => MemTileMembership::None,
                TileEntry::Border(id) => MemTileMembership::Border(id),
            })
            .collect();

        let border_store = BorderStore::from_compressed(gd.border_blobs);

        let entities: HashMap<GeoEntityId, GeoEntity> =
            gd.entities.into_iter().map(|e| (e.id, e)).collect();

        Ok(Self {
            tile_lookup,
            border_store,
            entities,
            worldviews: gd.worldviews,
            provenance_hash: gd.provenance_hash,
        })
    }

    pub fn lookup(
        &self,
        tile_x: u16,
        tile_y: u16,
        block_x: u8,
        block_y: u8,
    ) -> Option<GeoEntityId> {
        debug_assert!(
            (block_x as i64) < TILE_WIDTH && (block_y as i64) < TILE_WIDTH,
            "block coord out of range: ({block_x}, {block_y})"
        );
        let tile_idx = tile_y as usize * MAP_WIDTH as usize + tile_x as usize;
        match &self.tile_lookup[tile_idx] {
            MemTileMembership::Single(v) => Some(*v),
            MemTileMembership::None => None,
            MemTileMembership::Border(id) => {
                let cell_idx = block_y as usize * TILE_WIDTH as usize + block_x as usize;
                self.border_store.lookup(*id, cell_idx)
            }
        }
    }

    pub fn get_entity(&self, id: GeoEntityId) -> Option<&GeoEntity> {
        self.entities.get(&id)
    }

    pub fn entity_kind(&self, id: GeoEntityId) -> Option<GeoEntityKind> {
        self.entities.get(&id).map(|e| e.kind)
    }

    pub fn ancestor_of_kind(
        &self,
        entity_id: GeoEntityId,
        kind: GeoEntityKind,
    ) -> Option<GeoEntityId> {
        let entity = self.entities.get(&entity_id)?;
        if entity.kind == kind {
            return Some(entity_id);
        }
        match entity.parent_id {
            Some(parent_id) => self.ancestor_of_kind(parent_id, kind),
            None => None,
        }
    }

    pub fn entities_of_kind(&self, kind: GeoEntityKind) -> Vec<&GeoEntity> {
        self.entities.values().filter(|e| e.kind == kind).collect()
    }

    pub fn worldviews(&self) -> &[Worldview] {
        &self.worldviews
    }

    pub fn provenance_hash(&self) -> [u8; 32] {
        self.provenance_hash
    }

    /// Resident heap bytes held by the border-tile store. For diagnostics.
    pub fn border_resident_heap_bytes(&self) -> usize {
        self.border_store.resident_heap_bytes()
    }

    #[doc(hidden)]
    pub fn empty_for_test() -> Self {
        Self {
            tile_lookup: vec![MemTileMembership::None; (MAP_WIDTH * MAP_WIDTH) as usize],
            border_store: BorderStore::build(Vec::new()),
            entities: HashMap::new(),
            worldviews: vec![],
            provenance_hash: [0u8; 32],
        }
    }

    #[doc(hidden)]
    pub fn fixture(
        entities: Vec<super::geo_entity::GeoEntity>,
        tiles: Vec<((u16, u16), TileMembership)>,
    ) -> Self {
        let mut tile_lookup = vec![MemTileMembership::None; (MAP_WIDTH * MAP_WIDTH) as usize];
        for ((tx, ty), membership) in tiles {
            let idx = ty as usize * MAP_WIDTH as usize + tx as usize;
            tile_lookup[idx] = match membership {
                TileMembership::Single(v) => MemTileMembership::Single(v),
                TileMembership::None => MemTileMembership::None,
                TileMembership::Border => {
                    panic!(
                        "GeoLookupTable::fixture does not support Border tiles; use new_synthetic"
                    )
                }
            };
        }
        let entities_map: HashMap<GeoEntityId, super::geo_entity::GeoEntity> =
            entities.into_iter().map(|e| (e.id, e)).collect();
        Self {
            tile_lookup,
            border_store: BorderStore::build(Vec::new()),
            entities: entities_map,
            worldviews: vec![],
            provenance_hash: [0u8; 32],
        }
    }

    /// Test-only constructor. Bypasses `load_from_bytes` and the
    /// production deserialization. Used by `synth_worldview` to assemble
    /// a small in-memory lookup table for property tests.
    ///
    /// The existing `fixture()` hard-codes `border_store: BorderStore::build(Vec::new())`
    /// and is unsuitable for the border-tile path. `new_synthetic` accepts a fully
    /// populated `block_lookup` and is the only way to exercise `TileMembership::Border`.
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn new_synthetic(
        tile_lookup: Vec<TileMembership>,
        block_lookup: HashMap<(u16, u16), Vec<Option<GeoEntityId>>>,
        entities: HashMap<GeoEntityId, GeoEntity>,
        worldviews: Vec<Worldview>,
    ) -> Self {
        // Mirror load_from_bytes: hoist border tiles into a vector, rewrite
        // tile_lookup with the minted ids.
        let mut border_tiles: Vec<Vec<Option<GeoEntityId>>> = Vec::new();
        let mut border_id_by_coord: HashMap<(u16, u16), u32> = HashMap::new();
        for ((tx, ty), cells) in block_lookup {
            let id = border_tiles.len() as u32;
            border_id_by_coord.insert((tx, ty), id);
            border_tiles.push(cells);
        }
        let tile_lookup: Vec<MemTileMembership> = tile_lookup
            .into_iter()
            .enumerate()
            .map(|(idx, m)| match m {
                TileMembership::Single(v) => MemTileMembership::Single(v),
                TileMembership::None => MemTileMembership::None,
                TileMembership::Border => {
                    let tx = (idx as u32 % MAP_WIDTH as u32) as u16;
                    let ty = (idx as u32 / MAP_WIDTH as u32) as u16;
                    let id = *border_id_by_coord
                        .get(&(tx, ty))
                        .expect("border tile must have block_lookup entry");
                    MemTileMembership::Border(id)
                }
            })
            .collect();
        Self {
            tile_lookup,
            border_store: BorderStore::build(border_tiles),
            entities,
            worldviews,
            provenance_hash: [0u8; 32],
        }
    }
}

/// Test-support helpers available to sibling modules under `#[cfg(test)]`.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use geo_data_format::{
        write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
        CELLS_PER_TILE,
    };
    use std::collections::BTreeMap;

    /// Build the smallest valid geo-data binary understood by
    /// `GeoLookupTable::load_from_bytes`. Mirrors the fixture used by
    /// `load_from_bytes_round_trips`; `hash` is baked into the header so
    /// callers can distinguish two distinct blobs.
    pub fn tiny_geo_bin(hash: [u8; 32]) -> Vec<u8> {
        let mut tile_lookup = vec![TileMembership::None; (MAP_WIDTH * MAP_WIDTH) as usize];
        tile_lookup[0] = TileMembership::Single(GeoEntityId(0));
        tile_lookup[1] = TileMembership::Border;
        let mut cells = vec![None; CELLS_PER_TILE];
        cells[0] = Some(GeoEntityId(0));
        let mut block_lookup: BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>> = BTreeMap::new();
        block_lookup.insert((1, 0), cells);
        let entities = vec![GeoEntity {
            id: GeoEntityId(0),
            kind: GeoEntityKind::Country,
            iso_code: "TEST".to_string(),
            name_key: "country.TEST.name".to_string(),
            parent_id: None,
            total_area_m2: 1_000_000,
        }];
        let worldviews = vec![Worldview {
            id: "iso".to_string(),
            name_key: "wv".to_string(),
            description_key: "wv".to_string(),
        }];
        write_geo_data(&entities, &worldviews, &tile_lookup, &block_lookup, hash).expect("write ok")
    }
}

#[cfg(test)]
mod provenance_hash_tests {
    use super::*;

    #[test]
    fn empty_for_test_uses_zero_hash() {
        let t = GeoLookupTable::empty_for_test();
        assert_eq!(t.provenance_hash(), [0u8; 32]);
    }
}

#[cfg(test)]
mod load_from_bytes_tests {
    use super::*;
    use geo_data_format::GeoEntityId;

    fn small_bytes(hash: [u8; 32]) -> Vec<u8> {
        super::test_support::tiny_geo_bin(hash)
    }

    #[test]
    fn load_from_bytes_round_trips() {
        let bytes = small_bytes([9u8; 32]);
        let table = GeoLookupTable::load_from_bytes(&bytes).expect("should load");
        assert_eq!(table.provenance_hash(), [9u8; 32]);
        // Single tile.
        assert_eq!(table.lookup(0, 0, 0, 0), Some(GeoEntityId(0)));
        // Border tile (tx=1, ty=0), cell 0 → resolves via BorderStore.
        assert_eq!(table.lookup(1, 0, 0, 0), Some(GeoEntityId(0)));
        // Border tile, an unset cell → None.
        assert_eq!(table.lookup(1, 0, 1, 0), None);
    }

    #[test]
    fn load_from_bytes_rejects_bad_magic() {
        let mut bytes = small_bytes([0u8; 32]);
        bytes[0] = b'X';
        assert!(GeoLookupTable::load_from_bytes(&bytes).is_err());
    }
}
