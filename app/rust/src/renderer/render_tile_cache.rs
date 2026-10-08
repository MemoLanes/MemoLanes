use journey_kernel::bitmap2d::BitMap2D;
use std::collections::HashMap;

use super::SOURCE_TILE_ZOOM;
use crate::journey_bitmap::TileKey;

// Per-renderer limits, not a total map memory budget. The byte limit covers
// owned base/LOD bitmap allocations; the entry limit also bounds empty tiles.
// These are conservative defaults, not measured optimal working-set sizes.
const RENDER_TILE_CACHE_MAX_ENTRIES: usize = 128;
const RENDER_TILE_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) struct RenderTileCacheKey {
    pub(super) x: i64,
    pub(super) y: i64,
    pub(super) z: i16,
    pub(super) buffer_size_power: i16,
}

pub(super) enum CachedRenderTile {
    Empty,
    Bitmap(BitMap2D),
}

impl CachedRenderTile {
    /// Empty tiles own no bitmap; non-empty tiles always have complete LODs.
    pub(super) fn from_bitmap(mut bitmap: BitMap2D) -> Self {
        if bitmap.is_empty() {
            Self::Empty
        } else {
            bitmap.build_lods();
            Self::Bitmap(bitmap)
        }
    }

    pub(super) fn as_bitmap(&self) -> Option<&BitMap2D> {
        match self {
            Self::Empty => None,
            Self::Bitmap(bitmap) => Some(bitmap),
        }
    }

    fn storage_byte_len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Bitmap(bitmap) => bitmap.storage_byte_len(),
        }
    }
}

struct RenderTileCacheEntry {
    tile: CachedRenderTile,
    byte_len: usize,
    last_access: u64,
}

#[derive(Default)]
pub(super) struct RenderTileCache {
    entries: HashMap<RenderTileCacheKey, RenderTileCacheEntry>,
    byte_len: usize,
    access_clock: u64,
}

impl RenderTileCache {
    pub(super) fn get(&mut self, key: &RenderTileCacheKey) -> Option<&CachedRenderTile> {
        let entry = self.entries.get_mut(key)?;
        self.access_clock = self.access_clock.wrapping_add(1);
        entry.last_access = self.access_clock;
        Some(&entry.tile)
    }

    pub(super) fn insert(&mut self, key: RenderTileCacheKey, tile: CachedRenderTile) {
        let byte_len = tile.storage_byte_len();
        if byte_len > RENDER_TILE_CACHE_MAX_BYTES {
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.byte_len = self.byte_len.saturating_sub(old.byte_len);
        }
        while self.entries.len() >= RENDER_TILE_CACHE_MAX_ENTRIES
            || self.byte_len.saturating_add(byte_len) > RENDER_TILE_CACHE_MAX_BYTES
        {
            let Some(evict_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&evict_key) {
                self.byte_len = self.byte_len.saturating_sub(evicted.byte_len);
            }
        }

        self.access_clock = self.access_clock.wrapping_add(1);
        self.byte_len = self.byte_len.saturating_add(byte_len);
        self.entries.insert(
            key,
            RenderTileCacheEntry {
                tile,
                byte_len,
                last_access: self.access_clock,
            },
        );
    }

    pub(super) fn invalidate_changed_sources(&mut self, changed_tiles: &[TileKey]) {
        if changed_tiles.is_empty() || self.entries.is_empty() {
            return;
        }

        // Invalidate every resolution that depends on a changed source tile,
        // including cached empty results. Unrelated regions remain reusable.
        let mut removed_bytes = 0usize;
        self.entries.retain(|key, entry| {
            let keep = !changed_tiles
                .iter()
                .any(|source| render_tile_overlaps_source(key, *source));
            if !keep {
                removed_bytes = removed_bytes.saturating_add(entry.byte_len);
            }
            keep
        });
        self.byte_len = self.byte_len.saturating_sub(removed_bytes);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.byte_len = 0;
    }
}

fn render_tile_overlaps_source(key: &RenderTileCacheKey, source: TileKey) -> bool {
    let source_x = i64::from(source.x);
    let source_y = i64::from(source.y);
    let source_width = 1i64 << SOURCE_TILE_ZOOM;
    if !(0..source_width).contains(&source_x) || !(0..source_width).contains(&source_y) {
        return false;
    }

    if key.z >= SOURCE_TILE_ZOOM {
        let shift = (key.z - SOURCE_TILE_ZOOM) as u32;
        return key.x >> shift == source_x && key.y >> shift == source_y;
    }

    let shift = (SOURCE_TILE_ZOOM - key.z) as u32;
    source_x >> shift == key.x && source_y >> shift == key.y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: i64, buffer_size_power: i16) -> RenderTileCacheKey {
        RenderTileCacheKey {
            x,
            y: 0,
            z: SOURCE_TILE_ZOOM,
            buffer_size_power,
        }
    }

    #[test]
    fn empty_results_use_no_bitmap_storage_and_still_obey_lru_entry_limit() {
        let mut cache = RenderTileCache::default();
        for x in 0..RENDER_TILE_CACHE_MAX_ENTRIES as i64 {
            cache.insert(key(x, 10), CachedRenderTile::from_bitmap(BitMap2D::new(10)));
        }
        assert!(cache.get(&key(0, 10)).is_some());

        let newest = key(RENDER_TILE_CACHE_MAX_ENTRIES as i64, 10);
        cache.insert(newest, CachedRenderTile::from_bitmap(BitMap2D::new(10)));

        assert_eq!(cache.byte_len, 0);
        assert_eq!(cache.entries.len(), RENDER_TILE_CACHE_MAX_ENTRIES);
        assert!(cache.get(&key(0, 10)).is_some());
        assert!(cache.get(&key(1, 10)).is_none());
        assert!(cache.get(&newest).is_some());
    }

    #[test]
    fn bitmap_budget_evicts_old_tiles_and_reclaims_invalidated_storage() {
        let mut bitmap = BitMap2D::new(11);
        bitmap.set(0, 0, true);
        bitmap.build_lods();
        let tile_bytes = bitmap.storage_byte_len();
        let capacity = RENDER_TILE_CACHE_MAX_BYTES / tile_bytes;
        assert!(capacity < RENDER_TILE_CACHE_MAX_ENTRIES);

        let mut cache = RenderTileCache::default();
        for x in 0..=capacity as i64 {
            cache.insert(key(x, 11), CachedRenderTile::Bitmap(bitmap.clone()));
            assert!(cache.byte_len <= RENDER_TILE_CACHE_MAX_BYTES);
        }
        assert!(cache.get(&key(0, 11)).is_none());
        assert!(cache.get(&key(capacity as i64, 11)).is_some());

        let before = cache.byte_len;
        cache.insert(key(capacity as i64, 11), CachedRenderTile::Empty);
        cache.invalidate_changed_sources(&[TileKey::new(capacity as u16 - 1, 0)]);
        assert_eq!(cache.byte_len, before - 2 * tile_bytes);
        cache.clear();
        assert_eq!(cache.byte_len, 0);
        assert!(cache.entries.is_empty());
    }
}
