use std::sync::Arc;

use chrono::NaiveDate;

use super::geo_entity::{GeoEntity, GeoEntityId, GeoEntityKind};
use super::geo_lookup::GeoLookupTable;
use crate::journey_bitmap::JourneyBitmap;

/// Stable identifier for a NamedRegion. Two ID spaces:
///   GeoEntity: countries/provinces/cities from GeoLookupTable
///   Poi:       curated point/polygon collections (rasterized at build time)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RegionId {
    GeoEntity(GeoEntityId),
    Poi { list_id: String, item_id: u32 },
}

/// How a region's spatial extent is represented. The coverage primitive
/// dispatches on this internally.
#[derive(Debug, Clone)]
pub enum RegionFootprint {
    /// Region defined by entries in the GeoLookupTable. Implicit footprint:
    /// the set of (tile, block) cells that lookup back to this entity_id
    /// after worldview resolution.
    GeoLookup(GeoEntityId),

    /// Region defined by a pre-rasterized JourneyBitmap (POI footprints).
    /// Sparse — usually 1–50 set bits.
    ///
    /// Known memory hazard: JourneyBitmap's Tile is ~131KB regardless of
    /// occupancy. For 1000+ POIs, consider a SparseBitmap representation.
    /// Acceptable to defer because POI is D2.
    Bitmap(Arc<JourneyBitmap>),
}

/// A named geographic footprint that we can ask "did the user visit this?"
/// about. Both geo entities and POIs use this type.
#[derive(Debug, Clone)]
pub struct NamedRegion {
    pub id: RegionId,
    pub name_key: String,
    pub footprint: RegionFootprint,
    pub total_area_m2: u64,
}

/// Per-region result of a coverage query.
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    pub region_id: RegionId,
    pub covered_area_m2: u64,
    pub total_area_m2: u64,
    pub first_visited: Option<NaiveDate>,
}

impl Coverage {
    /// A region is "visited" iff `covered_area_m2 > 0`. No threshold.
    pub fn visited(&self) -> bool {
        self.covered_area_m2 > 0
    }

    /// Coverage as fraction in [0.0, 1.0]. Returns 0.0 for zero-area regions.
    pub fn percent(&self) -> f64 {
        if self.total_area_m2 == 0 {
            0.0
        } else {
            self.covered_area_m2 as f64 / self.total_area_m2 as f64
        }
    }
}

/// Build the list of `NamedRegion`s for all entities of a given kind.
/// Returns regions whose footprint is `RegionFootprint::GeoLookup`.
///
/// For `GeoEntityKind::City`: returns whatever the lookup table contains
/// (currently empty until the rasterizer emits city-level entries).
pub fn geo_regions_of_kind(lookup: &GeoLookupTable, kind: GeoEntityKind) -> Vec<NamedRegion> {
    lookup
        .entities_of_kind(kind)
        .into_iter()
        .map(|e: &GeoEntity| NamedRegion {
            id: RegionId::GeoEntity(e.id),
            name_key: e.name_key.clone(),
            footprint: RegionFootprint::GeoLookup(e.id),
            total_area_m2: e.total_area_m2,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::achievement::geo_entity::GeoEntityKind;
    use crate::achievement::test_strategies::{synth_worldview, SynthParams};
    use proptest::prelude::*;

    proptest! {
        // §8.4 filter correctness: only entities of kind K appear.
        #[test]
        fn filter_correctness(_seed in any::<u8>()) {
            let fixture = synth_worldview(&SynthParams::plain());
            for kind in [GeoEntityKind::Continent, GeoEntityKind::Country] {
                let regions = geo_regions_of_kind(&fixture.lookup, kind);
                for r in &regions {
                    if let RegionId::GeoEntity(eid) = &r.id {
                        let entity_kind = fixture.lookup.entity_kind(*eid).unwrap();
                        prop_assert_eq!(entity_kind, kind);
                    } else {
                        prop_assert!(false, "geo_regions_of_kind must return GeoEntity ids");
                    }
                }
            }
        }

        // §8.4 footprint shape: all are GeoLookup.
        #[test]
        fn footprint_geo_lookup(_seed in any::<u8>()) {
            let fixture = synth_worldview(&SynthParams::plain());
            for kind in [GeoEntityKind::Continent, GeoEntityKind::Country] {
                let regions = geo_regions_of_kind(&fixture.lookup, kind);
                for r in &regions {
                    prop_assert!(matches!(r.footprint, RegionFootprint::GeoLookup(_)));
                }
            }
        }

        // §8.4 ID round-trip: region.id matches inner GeoEntity id.
        #[test]
        fn id_round_trip(_seed in any::<u8>()) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = geo_regions_of_kind(&fixture.lookup, GeoEntityKind::Country);
            for r in &regions {
                if let RegionFootprint::GeoLookup(eid) = &r.footprint {
                    prop_assert_eq!(&r.id, &RegionId::GeoEntity(*eid));
                }
            }
        }
    }
}
