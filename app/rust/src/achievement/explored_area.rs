//! Explored-area read-model: total covered area (m²) per layer. Pure — depends
//! only on the layer scope, storage, and the bitmap-area primitive.

use std::collections::HashMap;

use anyhow::Result;

use super::scope::AchievementLayer;
use crate::journey_area_utils::compute_journey_bitmap_area;
use crate::storage::Storage;

/// Explored area (m²) for each requested layer, read under one consistent
/// journey snapshot so the layers cannot skew against each other.
pub fn compute_explored_areas(
    storage: &Storage,
    layers: &[AchievementLayer],
) -> Result<HashMap<AchievementLayer, u64>> {
    storage.with_journey_snapshot(|snapshot| {
        let mut out = HashMap::with_capacity(layers.len());
        for &layer in layers {
            let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
            out.insert(layer, compute_journey_bitmap_area(&bitmap, None));
        }
        Ok(out)
    })
}
