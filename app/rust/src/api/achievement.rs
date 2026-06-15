use anyhow::Result;
use flutter_rust_bridge::frb;

pub use crate::achievement::scope::AchievementLayer;

/// Total explored area for one layer in a single struct, for the
/// "Default vs Flight vs All" comparison row.
#[frb(non_opaque)]
#[derive(Debug, Clone)]
pub struct LayerArea {
    pub layer: AchievementLayer,
    pub area_m2: u64,
}

/// Total explored area per layer. Cheap: three cached-bitmap fetches
/// (one consistent storage snapshot, so the rows can never contradict
/// each other, e.g. All < Default) and area sums — no first_visited
/// involvement, no worldview dependency (area is computed from journey
/// bitmaps alone).
pub fn get_explored_area_by_layer() -> Result<Vec<LayerArea>> {
    let storage = &crate::api::api::get().storage;
    storage.with_journey_snapshot(|snapshot| {
        AchievementLayer::ALL_LAYERS
            .into_iter()
            .map(|layer| {
                let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
                Ok(LayerArea {
                    layer,
                    area_m2: crate::journey_area_utils::compute_journey_bitmap_area(&bitmap, None),
                })
            })
            .collect()
    })
}
