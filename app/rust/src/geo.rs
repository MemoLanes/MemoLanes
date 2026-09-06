//! Runtime geo lookup: map a `JourneyBitmap` block to its owning geo entity
//! over the packed `geo_data_format` asset, decode-on-demand. One asset per worldview.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use geo_data_format::{
    tile_index, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile, TileEntry,
    TileMembership, TILE_GRID_WIDTH,
};

use crate::journey_bitmap::{BlockKey, TileKey};

#[derive(Debug)]
pub struct GeoAssetError(anyhow::Error);

impl std::fmt::Display for GeoAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "geo asset unreadable: {}", self.0)
    }
}

impl std::error::Error for GeoAssetError {}

impl GeoAssetError {
    pub fn is_in(error: &anyhow::Error) -> bool {
        error.chain().any(|cause| cause.is::<GeoAssetError>())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoNode {
    pub kind: GeoEntityKind,
    pub parent_id: Option<GeoEntityId>,
}

pub trait GeoLookup {
    /// The entity owning a block, or `None` over ocean. Errors only when the
    /// backing asset cannot be read.
    fn entity_of_block(&self, tile: TileKey, block: BlockKey) -> Result<Option<GeoEntityId>>;
    fn tile_membership(&self, tile: TileKey) -> TileMembership;
    fn node(&self, id: GeoEntityId) -> Option<GeoNode>;
    fn describe(&self, ids: &[GeoEntityId]) -> Result<HashMap<GeoEntityId, GeoEntity>>;
    fn entities_of_kind(&self, kind: GeoEntityKind) -> &[GeoEntityId];
    /// Ancestors from `id`'s parent to the root, nearest first.
    fn ancestors(&self, id: GeoEntityId) -> Vec<GeoEntityId>;
    /// Direct children of `id` (one level down).
    fn children(&self, id: GeoEntityId) -> &[GeoEntityId];
    fn provenance_hash(&self) -> [u8; 32];
}

pub struct GeoIndex {
    data: GeoData,
    nodes: Vec<Option<GeoNode>>,
    by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>>,
    child_offsets: Vec<u32>,
    child_ids: Vec<GeoEntityId>,
    decoded: Mutex<Option<(u32, PackedTile)>>,
}

impl GeoIndex {
    /// border tiles on disk, decoded on demand
    pub fn open(path: &Path) -> Result<Self> {
        Self::new(GeoData::open(path)?)
    }

    fn new(data: GeoData) -> Result<Self> {
        let entities = data.entities()?;
        let slots = entities
            .iter()
            .map(|e| e.id.0)
            .max()
            .map_or(0, |m| m as usize + 1);
        let mut nodes = vec![None; slots];
        let mut by_kind: HashMap<GeoEntityKind, Vec<GeoEntityId>> = HashMap::new();
        let mut child_offsets = vec![0u32; slots + 1];
        for e in &entities {
            nodes[e.id.0 as usize] = Some(GeoNode {
                kind: e.kind,
                parent_id: e.parent_id,
            });
            by_kind.entry(e.kind).or_default().push(e.id);
            if let Some(parent) = e.parent_id {
                child_offsets[parent.0 as usize + 1] += 1;
            }
        }
        for i in 1..child_offsets.len() {
            child_offsets[i] += child_offsets[i - 1];
        }
        let mut cursor = child_offsets.clone();
        let mut child_ids = vec![GeoEntityId(0); child_offsets[slots] as usize];
        for e in &entities {
            if let Some(parent) = e.parent_id {
                let slot = &mut cursor[parent.0 as usize];
                child_ids[*slot as usize] = e.id;
                *slot += 1;
            }
        }
        Ok(GeoIndex {
            data,
            nodes,
            by_kind,
            child_offsets,
            child_ids,
            decoded: Mutex::new(None),
        })
    }

    fn tile_entry(&self, tile: TileKey) -> TileEntry {
        if tile.x as usize >= TILE_GRID_WIDTH || tile.y as usize >= TILE_GRID_WIDTH {
            return TileEntry::None;
        }
        self.data.tile_index.get(tile_index(tile.x, tile.y))
    }

    pub fn worldview_id(&self) -> &str {
        &self.data.worldview_id
    }
}

impl GeoLookup for GeoIndex {
    fn entity_of_block(&self, tile: TileKey, block: BlockKey) -> Result<Option<GeoEntityId>> {
        match self.tile_entry(tile) {
            TileEntry::None => Ok(None),
            TileEntry::Single(id) => Ok(Some(id)),
            TileEntry::Border(i) => {
                let mut slot = self
                    .decoded
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if slot.as_ref().is_none_or(|(idx, _)| *idx != i) {
                    let packed = self
                        .data
                        .border_blobs
                        .get(i)
                        .and_then(|blob| PackedTile::from_compressed_bytes(&blob))
                        .map_err(GeoAssetError)?;
                    *slot = Some((i, packed));
                }
                // `BlockKey::index()` is the x-major cell index PackedTile expects.
                let (_, packed) = slot.as_ref().expect("just populated");
                Ok(packed.lookup(block.index()))
            }
        }
    }

    fn tile_membership(&self, tile: TileKey) -> TileMembership {
        match self.tile_entry(tile) {
            TileEntry::None => TileMembership::None,
            TileEntry::Single(id) => TileMembership::Single(id),
            TileEntry::Border(_) => TileMembership::Border,
        }
    }

    fn node(&self, id: GeoEntityId) -> Option<GeoNode> {
        self.nodes.get(id.0 as usize).copied().flatten()
    }

    fn describe(&self, ids: &[GeoEntityId]) -> Result<HashMap<GeoEntityId, GeoEntity>> {
        let wanted: HashSet<GeoEntityId> = ids.iter().copied().collect();
        Ok(self
            .data
            .entities()?
            .into_iter()
            .filter(|e| wanted.contains(&e.id))
            .map(|e| (e.id, e))
            .collect())
    }

    fn entities_of_kind(&self, kind: GeoEntityKind) -> &[GeoEntityId] {
        self.by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    fn ancestors(&self, id: GeoEntityId) -> Vec<GeoEntityId> {
        let mut out = Vec::new();
        let mut cur = self.node(id).and_then(|n| n.parent_id);
        while let Some(pid) = cur {
            out.push(pid);
            cur = self.node(pid).and_then(|n| n.parent_id);
        }
        out
    }

    fn children(&self, id: GeoEntityId) -> &[GeoEntityId] {
        let i = id.0 as usize;
        match self.child_offsets.get(i..=i + 1) {
            Some(&[start, end]) => &self.child_ids[start as usize..end as usize],
            _ => &[],
        }
    }

    fn provenance_hash(&self) -> [u8; 32] {
        self.data.provenance_hash
    }
}
