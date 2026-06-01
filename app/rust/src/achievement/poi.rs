//! POI list types. Loader (`load_poi_list_from_bytes`) is stubbed `todo!()`
//! until POI data files exist. The types and `as_named_regions()` helper
//! are functional so `composites::poi_list_completion` (Task 14) compiles
//! and runs against in-memory test fixtures.

use std::sync::Arc;

use crate::journey_bitmap::JourneyBitmap;

use super::region::{NamedRegion, RegionFootprint, RegionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoiId(pub u32);

#[derive(Debug, Clone)]
pub struct PoiItem {
    pub id: PoiId,
    pub name_key: String,
    /// Pre-rasterized footprint. Sparse — typically 1–50 set bits.
    pub footprint: Arc<JourneyBitmap>,
    pub total_area_m2: u64,
}

#[derive(Debug, Clone)]
pub struct PoiList {
    pub id: String,
    pub name_key: String,
    pub items: Vec<PoiItem>,
}

impl PoiList {
    /// Convert a POI list into the unified `NamedRegion` form so it can
    /// be passed through `coverage()` alongside geo entities.
    pub fn as_named_regions(&self) -> Vec<NamedRegion> {
        self.items
            .iter()
            .map(|i| NamedRegion {
                id: RegionId::Poi {
                    list_id: self.id.clone(),
                    item_id: i.id.0,
                },
                name_key: i.name_key.clone(),
                footprint: RegionFootprint::Bitmap(i.footprint.clone()),
                total_area_m2: i.total_area_m2,
            })
            .collect()
    }
}

/// Result type for the `poi_list_completion` composite.
#[derive(Debug, Clone)]
pub struct PoiListCompletion {
    pub list_id: String,
    pub total: u32,
    pub visited: u32,
    pub items: Vec<super::region::Coverage>,
}

/// Load a POI list from bundled bytes.
/// **D2 stub** — POI data files do not yet exist; rasterizer is out of
/// scope for this refactor. Returns `todo!()`.
#[allow(dead_code)]
pub fn load_poi_list_from_bytes(_data: &[u8]) -> anyhow::Result<PoiList> {
    todo!("POI rasterizer not implemented yet")
}

#[cfg(test)]
mod tests {
    // `arc_with_non_send_sync`: JourneyBitmap carries a RefCell mipmap cache
    // so Journey is not Sync. Arc is used for cheap refcount sharing within a
    // single computation, never across threads.
    #![allow(clippy::arc_with_non_send_sync)]
    use super::*;
    use crate::journey_bitmap::JourneyBitmap;
    use proptest::prelude::*;
    use std::sync::Arc;

    fn arb_poi_item() -> impl Strategy<Value = PoiItem> {
        (any::<u32>(), "[a-z]{1,8}", 0u64..1_000_000).prop_map(|(id, name, area)| PoiItem {
            id: PoiId(id),
            name_key: format!("poi.{name}"),
            footprint: Arc::new(JourneyBitmap::new()),
            total_area_m2: area,
        })
    }

    fn arb_poi_list() -> impl Strategy<Value = PoiList> {
        ("[a-z]{1,8}", prop::collection::vec(arb_poi_item(), 0..=8)).prop_map(|(list_id, items)| {
            PoiList {
                id: list_id.clone(),
                name_key: format!("list.{list_id}"),
                items,
            }
        })
    }

    proptest! {
        // §8.8 length preservation.
        #[test]
        fn length_preservation(list in arb_poi_list()) {
            let regions = list.as_named_regions();
            prop_assert_eq!(regions.len(), list.items.len());
        }

        // §8.8 all are Bitmap footprints.
        #[test]
        fn all_bitmap_footprints(list in arb_poi_list()) {
            let regions = list.as_named_regions();
            for r in &regions {
                prop_assert!(matches!(r.footprint, RegionFootprint::Bitmap(_)));
            }
        }

        // §8.8 ID + area round-trip per item.
        #[test]
        fn id_and_area_round_trip(list in arb_poi_list()) {
            let regions = list.as_named_regions();
            for (item, region) in list.items.iter().zip(regions.iter()) {
                match &region.id {
                    RegionId::Poi { list_id, item_id } => {
                        prop_assert_eq!(list_id, &list.id);
                        prop_assert_eq!(*item_id, item.id.0);
                    }
                    _ => prop_assert!(false, "expected RegionId::Poi"),
                }
                prop_assert_eq!(region.total_area_m2, item.total_area_m2);
            }
        }
    }
}
