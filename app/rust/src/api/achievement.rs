use anyhow::Result;
use flutter_rust_bridge::frb;

pub use crate::achievement::scope::AchievementLayer;
use crate::journey_snapshot::JourneySnapshot;

/// Total explored area for one layer in a single struct, for the
/// "Default vs Flight vs All" comparison row.
#[frb(non_opaque)]
#[derive(Debug, Clone)]
pub struct LayerArea {
    pub layer: AchievementLayer,
    pub area_m2: u64,
}

/// Total explored area (m²) for one layer, from its merged finalized
/// bitmap. Shared by the single-layer and per-layer queries so they
/// always agree.
fn layer_area_m2(snapshot: &JourneySnapshot, layer: AchievementLayer) -> Result<u64> {
    let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
    Ok(crate::journey_area_utils::compute_journey_bitmap_area(
        &bitmap, None,
    ))
}

/// Total explored area for a single layer. Cheap: one cached-bitmap
/// fetch and an area sum — no first_visited or worldview involvement.
pub fn get_explored_area(layer: AchievementLayer) -> Result<u64> {
    let storage = &crate::api::api::get().storage;
    storage.with_journey_snapshot(|snapshot| layer_area_m2(snapshot, layer))
}

/// Total explored area per layer, read as ONE consistent snapshot so the
/// rows can never contradict each other (e.g. `All` < `Default`). For the
/// Default-vs-Flight-vs-All comparison; for a single layer use
/// [`get_explored_area`].
pub fn get_explored_area_by_layer() -> Result<Vec<LayerArea>> {
    let storage = &crate::api::api::get().storage;
    storage.with_journey_snapshot(|snapshot| {
        AchievementLayer::ALL_LAYERS
            .into_iter()
            .map(|layer| {
                Ok(LayerArea {
                    layer,
                    area_m2: layer_area_m2(snapshot, layer)?,
                })
            })
            .collect()
    })
}
