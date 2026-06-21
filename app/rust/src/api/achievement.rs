use std::collections::HashMap;

use anyhow::Result;

use crate::achievement::explored_area::compute_explored_areas;
use crate::achievement::query::{AchievementQuery, AchievementValue};
pub use crate::achievement::scope::AchievementLayer;

/// Explored area (m²) per layer, via the stat cache. On any miss the WHOLE set
/// is recomputed under one snapshot (not just the misses), so cached and fresh
/// layers can't mix journey states (which would break `All >= Default`).
fn cached_explored_areas(layers: &[AchievementLayer]) -> Result<HashMap<AchievementLayer, u64>> {
    let state = crate::api::api::get();
    let cache = &state.stat_cache;

    let mut out = HashMap::with_capacity(layers.len());
    for &layer in layers {
        match cache.get(&AchievementQuery::ExploredAreaM2 { layer }) {
            Some(AchievementValue::U64(area)) => {
                out.insert(layer, area);
            }
            _ => {
                let computed = compute_explored_areas(&state.storage, layers)?;
                for (&layer, &area) in &computed {
                    cache.put(
                        AchievementQuery::ExploredAreaM2 { layer },
                        AchievementValue::U64(area),
                    );
                }
                return Ok(computed);
            }
        }
    }
    Ok(out)
}

/// Explored area for a single layer.
pub fn get_explored_area(layer: AchievementLayer) -> Result<u64> {
    Ok(cached_explored_areas(&[layer])?
        .get(&layer)
        .copied()
        .unwrap_or(0))
}

/// Explored area for every layer.
pub fn get_explored_area_by_layer() -> Result<HashMap<AchievementLayer, u64>> {
    cached_explored_areas(&AchievementLayer::ALL_LAYERS)
}
