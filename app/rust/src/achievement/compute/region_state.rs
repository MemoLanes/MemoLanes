//! Region state compute kernel: per-`(layer, entity)` visited area, intersecting
//! each layer's merged coverage bitmap with the geo lookup. Pure.
//!
//! Area-only: a region is bounded by *where*, not *when* — a spatial projection
//! of per-layer coverage onto the geo tree. Layering (which kinds union into
//! which layer) is `cache_db`'s job; callers hand in the merged bitmap per layer.

use std::collections::HashMap;

use geo_data_format::GeoEntityId;

use crate::achievement::layer::AchievementLayer;
use crate::geo::GeoLookup;
use crate::journey_area_utils::block_area_m2;
use crate::journey_bitmap::JourneyBitmap;

/// Per-`(layer, entity)` region coverage.
pub type RegionStateMap = HashMap<(AchievementLayer, GeoEntityId), RegionState>;

/// One entity's coverage within one layer.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionState {
    /// Latitude-corrected area covered within this layer.
    pub visited_area_m2: u64,
}

/// Compute the region states from each layer's merged coverage bitmap.
///
/// Each bitmap is already the layer's union (e.g. `cache_db`'s `Default` /
/// `Flight` / `All` layer), so this just attributes every visited block's
/// latitude-corrected area to its geo entity and that entity's ancestors.
pub fn compute_region_states(
    layer_bitmaps: impl IntoIterator<Item = (AchievementLayer, JourneyBitmap)>,
    geo: &dyn GeoLookup,
) -> RegionStateMap {
    let mut states = RegionStateMap::new();
    for (layer, bitmap) in layer_bitmaps {
        // Area per entity (and its ancestors) over this layer's coverage.
        let mut area_by_entity: HashMap<GeoEntityId, f64> = HashMap::new();
        let tile_keys: Vec<_> = bitmap.all_tile_keys().cloned().collect();
        for tile_key in &tile_keys {
            bitmap.peek_tile_without_updating_cache(tile_key, |tile| {
                if let Some(tile) = tile {
                    for (block_key, block) in tile.iter() {
                        let bit_count = block.count();
                        if bit_count == 0 {
                            continue;
                        }
                        if let Some(entity) = geo.entity_of_block(*tile_key, block_key) {
                            let area = block_area_m2(tile_key, &block_key, bit_count);
                            *area_by_entity.entry(entity).or_default() += area;
                            for ancestor in geo.ancestors(entity) {
                                *area_by_entity.entry(ancestor).or_default() += area;
                            }
                        }
                    }
                }
            });
        }

        for (entity, area) in area_by_entity {
            states.insert(
                (layer, entity),
                RegionState {
                    visited_area_m2: area.round() as u64,
                },
            );
        }
    }

    states
}
