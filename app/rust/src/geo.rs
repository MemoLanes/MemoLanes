//! Runtime geo lookup: map a `JourneyBitmap` block to its owning geo entity
//! over the packed `geo_data_format` asset, decode-on-demand. One asset per worldview.

use std::collections::HashMap;

use anyhow::Result;
use geo_data_format::{
    read_geo_data, tile_index, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile,
    TileEntry, TileMembership,
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
}

/// `GeoData`-backed lookup: tile index in memory, border tiles decoded on demand.
pub struct GeoIndex {
    data: GeoData,
    by_id: HashMap<GeoEntityId, GeoEntity>,
    by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>>,
    children: HashMap<GeoEntityId, Vec<GeoEntityId>>,
}

impl GeoIndex {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::new(read_geo_data(bytes)?)
    }

    pub fn new(data: GeoData) -> Result<Self> {
        let mut by_id = HashMap::with_capacity(data.entities.len());
        let mut by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>> = HashMap::new();
        let mut children: HashMap<GeoEntityId, Vec<GeoEntityId>> = HashMap::new();
        for e in &data.entities {
            by_kind.entry(e.kind).or_default().push(e.id);
            if let Some(parent) = e.parent_id {
                children.entry(parent).or_default().push(e.id);
            }
            by_id.insert(e.id, e.clone());
        }
        Ok(GeoIndex {
            data,
            by_id,
            by_kind,
            children,
        })
    }

    fn tile_entry(&self, tile: TileKey) -> &TileEntry {
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
            // Decode-on-demand; a least-recently-used cache over decoded tiles is a deferred optimization.
            TileEntry::Border(i) => {
                let packed =
                    PackedTile::from_compressed_bytes(&self.data.border_blobs[*i as usize]);
                // `BlockKey::index()` is the x-major cell index PackedTile expects.
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
        self.by_id.get(&id)
    }

    fn entities_of_kind(&self, kind: GeoEntityKind) -> &[GeoEntityId] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    fn ancestors(&self, id: GeoEntityId) -> Vec<GeoEntityId> {
        let mut out = Vec::new();
        let mut cur = self.by_id.get(&id).and_then(|e| e.parent_id);
        while let Some(pid) = cur {
            out.push(pid);
            cur = self.by_id.get(&pid).and_then(|e| e.parent_id);
        }
        out
    }

    fn children(&self, id: GeoEntityId) -> &[GeoEntityId] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }
}
