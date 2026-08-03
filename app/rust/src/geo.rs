//! Runtime geo lookup: map a `JourneyBitmap` block to its owning geo entity
//! over the packed `geo_data_format` asset, decode-on-demand. One asset per worldview.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;
use geo_data_format::{
    read_geo_data, tile_index, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile,
    TileEntry, TileMembership, TILE_GRID_WIDTH,
};

use crate::journey_bitmap::{BlockKey, TileKey};

pub trait GeoLookup {
    /// The entity owning a block, or `None` over ocean.
    fn entity_of_block(&self, tile: TileKey, block: BlockKey) -> Option<GeoEntityId>;
    fn tile_membership(&self, tile: TileKey) -> TileMembership;
    fn entity(&self, id: GeoEntityId) -> Option<&GeoEntity>;
    fn entities_of_kind(&self, kind: GeoEntityKind) -> &[GeoEntityId];
    /// Ancestors from `id`'s parent to the root, nearest first.
    fn ancestors(&self, id: GeoEntityId) -> Vec<GeoEntityId>;
    /// Direct children of `id` (one level down).
    fn children(&self, id: GeoEntityId) -> &[GeoEntityId];
    fn provenance_hash(&self) -> [u8; 32];
}

/// `GeoData`-backed lookup: tile index in memory, border tiles decoded on demand.
pub struct GeoIndex {
    data: GeoData,
    /// `entity id -> index into data.entities`, `None` where the id is absent
    /// from this worldview
    by_id: Vec<Option<u32>>,
    by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>>,
    children: HashMap<GeoEntityId, Vec<GeoEntityId>>,
    decoded: Mutex<Option<(u32, PackedTile)>>,
}

impl GeoIndex {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::new(read_geo_data(bytes)?)
    }

    pub fn new(data: GeoData) -> Result<Self> {
        let slots = data.entities.iter().map(|e| e.id.0).max().unwrap_or(0) as usize + 1;
        let mut by_id: Vec<Option<u32>> = vec![None; slots];
        let mut by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>> = HashMap::new();
        let mut children: HashMap<GeoEntityId, Vec<GeoEntityId>> = HashMap::new();
        for (index, e) in data.entities.iter().enumerate() {
            by_kind.entry(e.kind).or_default().push(e.id);
            if let Some(parent) = e.parent_id {
                children.entry(parent).or_default().push(e.id);
            }
            by_id[e.id.0 as usize] = Some(index as u32);
        }
        Ok(GeoIndex {
            data,
            by_id,
            by_kind,
            children,
            decoded: Mutex::new(None),
        })
    }

    fn tile_entry(&self, tile: TileKey) -> &TileEntry {
        if tile.x as usize >= TILE_GRID_WIDTH || tile.y as usize >= TILE_GRID_WIDTH {
            return &TileEntry::None;
        }
        &self.data.tile_index[tile_index(tile.x, tile.y)]
    }

    /// The worldview id this asset declares (see `GeoData::worldview_id`).
    pub fn worldview_id(&self) -> &str {
        &self.data.worldview_id
    }
}

impl GeoLookup for GeoIndex {
    fn entity_of_block(&self, tile: TileKey, block: BlockKey) -> Option<GeoEntityId> {
        match self.tile_entry(tile) {
            TileEntry::None => None,
            TileEntry::Single(id) => Some(*id),
            TileEntry::Border(i) => {
                let mut slot = self
                    .decoded
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if slot.as_ref().is_none_or(|(idx, _)| idx != i) {
                    let packed =
                        PackedTile::from_compressed_bytes(&self.data.border_blobs[*i as usize]);
                    *slot = Some((*i, packed));
                }
                // `BlockKey::index()` is the x-major cell index PackedTile expects.
                let (_, packed) = slot.as_ref().expect("just populated");
                packed.lookup(block.index())
            }
        }
    }

    fn tile_membership(&self, tile: TileKey) -> TileMembership {
        match self.tile_entry(tile) {
            TileEntry::None => TileMembership::None,
            TileEntry::Single(id) => TileMembership::Single(*id),
            TileEntry::Border(_) => TileMembership::Border,
        }
    }

    fn entity(&self, id: GeoEntityId) -> Option<&GeoEntity> {
        let index = self.by_id.get(id.0 as usize).copied().flatten()?;
        self.data.entities.get(index as usize)
    }

    fn entities_of_kind(&self, kind: GeoEntityKind) -> &[GeoEntityId] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    fn ancestors(&self, id: GeoEntityId) -> Vec<GeoEntityId> {
        let mut out = Vec::new();
        let mut cur = self.entity(id).and_then(|e| e.parent_id);
        while let Some(pid) = cur {
            out.push(pid);
            cur = self.entity(pid).and_then(|e| e.parent_id);
        }
        out
    }

    fn children(&self, id: GeoEntityId) -> &[GeoEntityId] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }

    fn provenance_hash(&self) -> [u8; 32] {
        self.data.provenance_hash
    }
}
