use std::collections::HashMap;

use anyhow::Result;
use geo_data_format::GeoEntityId;

use crate::geo::GeoLookup;
use crate::journey_area_utils::mercator_block_area_cm2;
use crate::journey_bitmap::{JourneyBitmap, TileKey};

pub type AreaCm2ByEntity = HashMap<GeoEntityId, i64>;

pub fn attribute(bitmap: &JourneyBitmap, geo: &dyn GeoLookup) -> Result<AreaCm2ByEntity> {
    let mut out = AreaCm2ByEntity::new();
    for tile_key in bitmap.all_tile_keys() {
        let tile_y = match *tile_key {
            TileKey::Mercator { y, .. } => y,
            TileKey::NorthPolar { .. } | TileKey::SouthPolar { .. } => continue,
        };
        bitmap.peek_tile_without_updating_cache(tile_key, |tile| -> Result<()> {
            let Some(tile) = tile else {
                return Ok(());
            };
            for (block_key, block) in tile.iter() {
                let bit_count = block.count();
                if bit_count == 0 {
                    continue;
                }
                let Some(entity) = geo.entity_of_block(*tile_key, block_key)? else {
                    continue;
                };
                let area_cm2 = mercator_block_area_cm2(tile_y, block_key.y(), bit_count);
                *out.entry(entity).or_default() += area_cm2;
                for ancestor in geo.ancestors(entity) {
                    *out.entry(ancestor).or_default() += area_cm2;
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}
