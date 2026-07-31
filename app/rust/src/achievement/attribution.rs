use std::collections::HashMap;

use geo_data_format::GeoEntityId;

use crate::geo::GeoLookup;
use crate::journey_area_utils::block_area_cm2;
use crate::journey_bitmap::JourneyBitmap;

pub type AreaCm2ByEntity = HashMap<GeoEntityId, i64>;

pub fn attribute(bitmap: &JourneyBitmap, geo: &dyn GeoLookup) -> AreaCm2ByEntity {
    let mut out = AreaCm2ByEntity::new();
    let tile_keys: Vec<_> = bitmap.all_tile_keys().cloned().collect();
    for tile_key in &tile_keys {
        bitmap.peek_tile_without_updating_cache(tile_key, |tile| {
            let Some(tile) = tile else {
                return;
            };
            for (block_key, block) in tile.iter() {
                let bit_count = block.count();
                if bit_count == 0 {
                    continue;
                }
                let Some(entity) = geo.entity_of_block(*tile_key, block_key) else {
                    continue;
                };
                let area_cm2 = block_area_cm2(tile_key, &block_key, bit_count);
                *out.entry(entity).or_default() += area_cm2;
                for ancestor in geo.ancestors(entity) {
                    *out.entry(ancestor).or_default() += area_cm2;
                }
            }
        });
    }
    out
}
