//! The coverage primitive — the heart of the achievement system.
//!
//! Algorithm:
//!
//! - Sort journeys chronologically by (date, id).
//! - Maintain a running merged JourneyBitmap.
//! - For each journey compute the bit-delta (bits in this journey not yet in
//!   the running bitmap). For GeoLookup regions attribute each new bit via
//!   the geo lookup. For Bitmap regions intersect with each POI footprint;
//!   on first overlap record first_visited.
//! - Merge the journey into the running bitmap.
//! - After the pass, compute covered_area_m2 per region.
//
// TODO(cache): for area-only/count composites (#1–#9 by area, #5), the
// merged bitmap is already maintained by `cache_db` (LayerKind::All) with
// auto-invalidation on journey writes. Add an `area_only` path that walks
// the cached bitmap once through GeoLookupTable and skips this entire
// chronological scan. Keep this fn for cold-rebuild + first_visited
// (#10, #31, #32) + POI overlap only.

use std::collections::HashMap;
use std::sync::Arc;

use crate::journey_area_utils::compute_journey_bitmap_area;
use crate::journey_bitmap::{
    Block, BlockKey, JourneyBitmap, Tile, TileKey, BITMAP_SIZE, BITMAP_WIDTH,
};

use super::geo_entity::GeoEntityId;
use super::geo_lookup::GeoLookupTable;
use super::journey::Journey;
use super::region::{Coverage, NamedRegion, RegionFootprint, RegionId};

/// Borrowed, read-only inputs shared across coverage-family composites in
/// a single query: the merged bitmap (from `cache_db` `LayerKind::All`)
/// and the all-time first_visited map (from `AchievementCache`). Replaces
/// the per-composite `coverage_from_journeys` rebuild.
pub struct CoverageCtx<'a> {
    pub bitmap: &'a JourneyBitmap,
    pub first_visited: &'a HashMap<RegionId, chrono::NaiveDate>,
}

/// Owned holder so callers (and the test-support builder) can keep the
/// merged bitmap + first_visited alive while handing out `CoverageCtx`.
pub struct OwnedCoverageInputs {
    pub bitmap: JourneyBitmap,
    pub first_visited: HashMap<RegionId, chrono::NaiveDate>,
}

impl OwnedCoverageInputs {
    pub fn ctx(&self) -> CoverageCtx<'_> {
        CoverageCtx {
            bitmap: &self.bitmap,
            first_visited: &self.first_visited,
        }
    }
}

/// Walk journeys chronologically by `(date, id)` and record, for each
/// region in `regions`, the date of the journey that first overlaps it.
///
/// Returns a map keyed by `RegionId` to the chronologically-first visit
/// date. Regions that are never visited do not appear in the returned map.
pub fn compute_first_visited(
    journeys: &[Arc<Journey>],
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> HashMap<RegionId, chrono::NaiveDate> {
    let mut sorted: Vec<&Arc<Journey>> = journeys.iter().collect();
    sorted.sort_by(|a, b| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())));

    let mut first_visited: HashMap<RegionId, chrono::NaiveDate> = HashMap::new();
    let mut running = JourneyBitmap::new();

    let geo_region_ids: Vec<(RegionId, GeoEntityId)> = regions
        .iter()
        .filter_map(|r| match &r.footprint {
            RegionFootprint::GeoLookup(eid) => Some((r.id.clone(), *eid)),
            RegionFootprint::Bitmap(_) => None,
        })
        .collect();
    let bitmap_regions: Vec<(RegionId, Arc<JourneyBitmap>)> = regions
        .iter()
        .filter_map(|r| match &r.footprint {
            RegionFootprint::Bitmap(bm) => Some((r.id.clone(), bm.clone())),
            RegionFootprint::GeoLookup(_) => None,
        })
        .collect();

    for j in &sorted {
        let j_bitmap = j.bitmap();
        if !geo_region_ids.is_empty() {
            attribute_new_bits_to_geo_regions(
                j_bitmap,
                &running,
                lookup,
                &geo_region_ids,
                &mut first_visited,
                j.date,
            );
        }
        for (rid, footprint) in &bitmap_regions {
            if first_visited.contains_key(rid) {
                continue;
            }
            if bitmaps_intersect(j_bitmap, footprint) {
                first_visited.insert(rid.clone(), j.date);
            }
        }
        running.merge_with_partial_clone(j_bitmap);
    }

    first_visited
}

/// Compute coverage for `regions` from a pre-merged `bitmap`.
///
/// `first_visited` is reflected verbatim into the returned
/// `Coverage::first_visited` field; pass `None` for area-only callers.
pub fn coverage(
    bitmap: &JourneyBitmap,
    first_visited: Option<&HashMap<RegionId, chrono::NaiveDate>>,
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> Vec<Coverage> {
    // Single-pass attribution: walk bitmap once, partition blocks by entity.
    let mut per_entity_bitmaps: HashMap<GeoEntityId, JourneyBitmap> = HashMap::new();
    let geo_region_present = regions
        .iter()
        .any(|r| matches!(r.footprint, RegionFootprint::GeoLookup(_)));
    if geo_region_present {
        let tile_keys: Vec<TileKey> = bitmap.all_tile_keys().copied().collect();
        for tile_pos in &tile_keys {
            let attributions: Vec<(GeoEntityId, BlockKey, Block)> = bitmap
                .peek_tile_without_updating_cache(tile_pos, |tile_opt| {
                    let mut out = Vec::new();
                    if let Some(tile) = tile_opt {
                        for (block_key, block) in tile.iter() {
                            let lookup_val =
                                lookup.lookup(tile_pos.x, tile_pos.y, block_key.x(), block_key.y());
                            if let Some(entity_id) = lookup_val {
                                out.push((entity_id, block_key, block.clone()));
                            }
                        }
                    }
                    out
                });
            for (entity_id, block_key, block) in attributions {
                let entity_bm = per_entity_bitmaps.entry(entity_id).or_default();
                let entity_tile = entity_bm.get_tile_mut_or_insert_empty(tile_pos);
                entity_tile.set(&block_key, block);
            }
        }
    }

    regions
        .iter()
        .map(|r| {
            let covered = match &r.footprint {
                RegionFootprint::GeoLookup(eid) => per_entity_bitmaps
                    .get(eid)
                    .map(|bm| compute_journey_bitmap_area(bm, None))
                    .unwrap_or(0),
                RegionFootprint::Bitmap(bm) => intersect_area(bitmap, bm),
            };
            Coverage {
                region_id: r.id.clone(),
                covered_area_m2: covered,
                total_area_m2: r.total_area_m2,
                first_visited: first_visited.and_then(|fv| fv.get(&r.id).copied()),
            }
        })
        .collect()
}

/// Convenience wrapper composing `compute_first_visited` + `coverage` from
/// a journey list. Used by composites until callers can supply
/// `(bitmap, first_visited)` directly.
pub fn coverage_from_journeys(
    journeys: &[Arc<Journey>],
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> Vec<Coverage> {
    // Build the merged bitmap once.
    let mut bitmap = JourneyBitmap::new();
    let mut sorted: Vec<&Arc<Journey>> = journeys.iter().collect();
    sorted.sort_by(|a, b| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())));
    for j in &sorted {
        bitmap.merge_with_partial_clone(j.bitmap());
    }
    let fv = compute_first_visited(journeys, regions, lookup);
    coverage(&bitmap, Some(&fv), regions, lookup)
}

/// Build `OwnedCoverageInputs` from a journey list the same way production
/// builds the two caches: merged bitmap over all journeys and all-kinds
/// first_visited map. Lets tests and benches exercise the
/// `coverage_from_journeys` → `CoverageCtx` seam without a live cache.
#[cfg(any(test, feature = "test-support"))]
pub fn coverage_inputs_from_journeys(
    journeys: &[Arc<Journey>],
    lookup: &GeoLookupTable,
) -> OwnedCoverageInputs {
    use super::geo_entity::GeoEntityKind;
    use super::region::geo_regions_of_kind;

    let mut bitmap = JourneyBitmap::new();
    let mut sorted: Vec<&Arc<Journey>> = journeys.iter().collect();
    sorted.sort_by(|a, b| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())));
    for j in &sorted {
        bitmap.merge_with_partial_clone(j.bitmap());
    }

    let mut all_regions = Vec::new();
    for kind in [
        GeoEntityKind::Continent,
        GeoEntityKind::Country,
        GeoEntityKind::Province,
        GeoEntityKind::City,
    ] {
        all_regions.extend(geo_regions_of_kind(lookup, kind));
    }
    let first_visited = compute_first_visited(journeys, &all_regions, lookup);

    OwnedCoverageInputs {
        bitmap,
        first_visited,
    }
}

/// For each set bit in `journey` NOT in `running`, look up the entity and
/// record first_visited for the matching region.
///
/// Per the current GeoLookupTable design, lookup returns at the BLOCK
/// granularity — the entire block belongs to one entity (or border, drilled
/// per-block). So attribution is effectively per-block, not per-bit.
/// We iterate set bits anyway for the "newly intersected" semantics.
fn attribute_new_bits_to_geo_regions(
    journey: &JourneyBitmap,
    running: &JourneyBitmap,
    lookup: &GeoLookupTable,
    geo_region_ids: &[(RegionId, GeoEntityId)],
    first_visited: &mut HashMap<RegionId, chrono::NaiveDate>,
    journey_date: chrono::NaiveDate,
) {
    // Build reverse map: GeoEntityId -> RegionId for quick attribution.
    let entity_to_region: HashMap<GeoEntityId, RegionId> = geo_region_ids
        .iter()
        .map(|(rid, eid)| (*eid, rid.clone()))
        .collect();

    // Walk every set bit in `journey` that isn't set in `running`.
    // For each such new bit, look up its entity and record first_visited.
    // BITMAP_WIDTH is i64; cast to usize for the loop bound.
    let bw = BITMAP_WIDTH as usize;
    let bits_per_block = bw * bw;

    let tile_keys: Vec<TileKey> = journey.all_tile_keys().copied().collect();
    for tile_pos in &tile_keys {
        // Collect RegionIds whose first_visited needs setting; record outside
        // the closure to keep first_visited's &mut borrow off the closure.
        let region_hits: Vec<RegionId> = journey.peek_tile_without_updating_cache(tile_pos, |jt| {
            let mut hits: Vec<RegionId> = Vec::new();
            let journey_tile = match jt {
                Some(t) => t,
                None => return hits,
            };
            running.peek_tile_without_updating_cache(tile_pos, |rt| {
                for (block_key, journey_block) in journey_tile.iter() {
                    let running_block = rt.and_then(|t| t.get(&block_key));
                    // Iterate set bits in journey_block not set in running_block.
                    for bit_idx in 0..bits_per_block {
                        let bx = (bit_idx % bw) as u8;
                        let by = (bit_idx / bw) as u8;
                        if !journey_block.is_visited(bx, by) {
                            continue;
                        }
                        if let Some(rb) = running_block {
                            if rb.is_visited(bx, by) {
                                continue;
                            }
                        }
                        // New bit. Look it up.
                        let lookup_val =
                            lookup.lookup(tile_pos.x, tile_pos.y, block_key.x(), block_key.y());
                        if let Some(entity_id) = lookup_val {
                            if let Some(rid) = entity_to_region.get(&entity_id) {
                                hits.push(rid.clone());
                            }
                        }
                    }
                }
            });
            hits
        });
        for rid in region_hits {
            first_visited.entry(rid).or_insert(journey_date);
        }
    }
}

/// Quick yes/no: do the two bitmaps share any set bit?
fn bitmaps_intersect(a: &JourneyBitmap, b: &JourneyBitmap) -> bool {
    for tile_pos in a.all_tile_keys() {
        if !b.contains_tile(tile_pos) {
            continue;
        }
        let intersects = a.peek_tile_without_updating_cache(tile_pos, |ta| {
            let Some(tile_a) = ta else { return false };
            b.peek_tile_without_updating_cache(tile_pos, |tb| {
                let Some(tile_b) = tb else { return false };
                for (block_key, block_a) in tile_a.iter() {
                    let Some(block_b) = tile_b.get(&block_key) else {
                        continue;
                    };
                    let a_data = block_a.raw_data();
                    let b_data = block_b.raw_data();
                    for byte_idx in 0..BITMAP_SIZE {
                        if (a_data[byte_idx] & b_data[byte_idx]) != 0 {
                            return true;
                        }
                    }
                }
                false
            })
        });
        if intersects {
            return true;
        }
    }
    false
}

/// Compute the area (m²) of `a ∩ b`.
fn intersect_area(a: &JourneyBitmap, b: &JourneyBitmap) -> u64 {
    let mut intersection = JourneyBitmap::new();
    let tile_keys: Vec<TileKey> = a.all_tile_keys().copied().collect();
    for tile_pos in &tile_keys {
        if !b.contains_tile(tile_pos) {
            continue;
        }
        // Collect intersected blocks for this tile pair, then assemble outside
        // the closures so we can mutate `intersection`.
        let int_blocks: Vec<(BlockKey, [u8; BITMAP_SIZE])> =
            a.peek_tile_without_updating_cache(tile_pos, |ta| {
                let Some(tile_a) = ta else { return Vec::new() };
                b.peek_tile_without_updating_cache(tile_pos, |tb| {
                    let Some(tile_b) = tb else { return Vec::new() };
                    let mut out: Vec<(BlockKey, [u8; BITMAP_SIZE])> = Vec::new();
                    for (block_key, block_a) in tile_a.iter() {
                        let Some(block_b) = tile_b.get(&block_key) else {
                            continue;
                        };
                        let a_data = block_a.raw_data();
                        let b_data = block_b.raw_data();
                        let mut int_data = [0u8; BITMAP_SIZE];
                        let mut block_any = false;
                        for ((dst, a_byte), b_byte) in
                            int_data.iter_mut().zip(a_data.iter()).zip(b_data.iter())
                        {
                            let v = a_byte & b_byte;
                            *dst = v;
                            if v != 0 {
                                block_any = true;
                            }
                        }
                        if block_any {
                            out.push((block_key, int_data));
                        }
                    }
                    out
                })
            });
        if !int_blocks.is_empty() {
            let mut int_tile = Tile::new();
            for (block_key, int_data) in int_blocks {
                int_tile.set(&block_key, Block::new_with_data(int_data));
            }
            intersection.insert_tile(tile_pos, int_tile);
        }
    }
    compute_journey_bitmap_area(&intersection, None)
}

#[cfg(test)]
mod tests {
    // `arc_with_non_send_sync`: JourneyBitmap carries a RefCell mipmap cache
    // so Journey is not Sync. Arc is used for cheap refcount sharing within a
    // single computation, never across threads.
    #![allow(clippy::arc_with_non_send_sync)]
    use super::*;
    use crate::achievement::geo_entity::GeoEntityKind;
    use crate::achievement::test_strategies::{
        arb_journey_list, arb_synth_params_mixed, naive_coverage, synth_worldview, Grid,
        SynthParams,
    };
    use proptest::prelude::*;
    use std::collections::{HashMap, HashSet};

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

        // §8.1 reference oracle — workhorse.
        #[test]
        fn coverage_matches_naive(
            params in arb_synth_params_mixed(),
            journeys in arb_journey_list(Grid::default_4x4()),
        ) {
            let fixture = synth_worldview(&params);
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let fast = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            let naive = naive_coverage(&journeys, &regions, &fixture.lookup);
            // Order may differ; compare as map keyed by region_id.
            let to_map = |v: Vec<Coverage>| -> HashMap<RegionId, Coverage> {
                v.into_iter().map(|c| (c.region_id.clone(), c)).collect()
            };
            prop_assert_eq!(to_map(fast), to_map(naive));
        }

        // §8.1 determinism.
        #[test]
        fn determinism(journeys in arb_journey_list(Grid::default_4x4())) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let a = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            let b = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            prop_assert_eq!(a, b);
        }

        // §8.1 permutation invariance — visited set + areas.
        #[test]
        fn permutation_invariance(journeys in arb_journey_list(Grid::default_4x4())) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let mut rev = journeys.clone();
            rev.reverse();
            let a = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            let b = coverage_from_journeys(&rev, &regions, &fixture.lookup);
            let visited_a: HashSet<RegionId> = a.iter().filter(|c| c.visited()).map(|c| c.region_id.clone()).collect();
            let visited_b: HashSet<RegionId> = b.iter().filter(|c| c.visited()).map(|c| c.region_id.clone()).collect();
            prop_assert_eq!(visited_a, visited_b);
            let areas_a: HashMap<_, _> = a.iter().map(|c| (c.region_id.clone(), c.covered_area_m2)).collect();
            let areas_b: HashMap<_, _> = b.iter().map(|c| (c.region_id.clone(), c.covered_area_m2)).collect();
            prop_assert_eq!(areas_a, areas_b);
        }

        // §8.1 monotonicity — adding a journey never shrinks visited set or area.
        #[test]
        fn monotonicity(
            mut journeys in arb_journey_list(Grid::default_4x4()),
            extra in arb_journey_list(Grid::default_4x4()),
        ) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let before = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            journeys.extend(extra);
            let after = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            for c_after in &after {
                if let Some(c_before) = before.iter().find(|c| c.region_id == c_after.region_id) {
                    prop_assert!(c_after.covered_area_m2 >= c_before.covered_area_m2);
                }
            }
        }

        // §8.1 idempotence — duplicating journeys doesn't change coverage.
        #[test]
        fn idempotence(journeys in arb_journey_list(Grid::default_4x4())) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let single = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            let mut doubled = journeys.clone();
            doubled.extend(journeys.iter().cloned());
            let dbl = coverage_from_journeys(&doubled, &regions, &fixture.lookup);
            for c in &single {
                let d = dbl.iter().find(|x| x.region_id == c.region_id).unwrap();
                prop_assert_eq!(c.covered_area_m2, d.covered_area_m2);
                prop_assert_eq!(&c.first_visited, &d.first_visited);
            }
        }

        // §8.1 containment.
        #[test]
        fn containment(journeys in arb_journey_list(Grid::default_4x4())) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let cov = coverage_from_journeys(&journeys, &regions, &fixture.lookup);
            for c in &cov {
                prop_assert!(c.covered_area_m2 <= c.total_area_m2);
            }
        }

        // §8.1 first_visited stability — appending a strictly-later journey
        // doesn't change existing first_visited entries.
        #[test]
        fn first_visited_stability(
            journeys in arb_journey_list(Grid::default_4x4()),
            late in arb_journey_list(Grid::default_4x4()),
        ) {
            // Force `late` journeys to be strictly after every existing one.
            let max_existing = journeys.iter().map(|j| j.date).max().unwrap_or_else(
                || chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
            let one_year_later = max_existing + chrono::Duration::days(366);
            let late_shifted: Vec<_> = late.into_iter().enumerate().map(|(i, j)| {
                std::sync::Arc::new(crate::achievement::journey::Journey::from_parts(
                    crate::achievement::journey::JourneyId(format!("{}.{}", j.id.as_str(), i)),
                    one_year_later + chrono::Duration::days(i as i64),
                    j.kind, j.start_time, j.end_time,
                    crate::journey_data::JourneyData::Bitmap(j.bitmap().clone()),
                ))
            }).collect();

            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let before = coverage_from_journeys(&journeys, &regions, &fixture.lookup);

            let mut combined = journeys.clone();
            combined.extend(late_shifted);
            let after = coverage_from_journeys(&combined, &regions, &fixture.lookup);

            for c_before in &before {
                if c_before.first_visited.is_some() {
                    let c_after = after.iter().find(|c| c.region_id == c_before.region_id).unwrap();
                    prop_assert_eq!(&c_before.first_visited, &c_after.first_visited);
                }
            }
        }

        // §8.1 empty input.
        #[test]
        fn empty_input(_dummy in any::<u8>()) {
            let fixture = synth_worldview(&SynthParams::plain());
            let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
            let cov = coverage_from_journeys(&[], &regions, &fixture.lookup);
            prop_assert!(cov.iter().all(|c| c.covered_area_m2 == 0));
            prop_assert!(cov.iter().all(|c| c.first_visited.is_none()));
        }
    }

    #[test]
    fn coverage_ctx_matches_coverage_from_journeys() {
        use crate::achievement::geo_entity::GeoEntityKind;
        use crate::achievement::test_strategies::{synth_worldview, SynthParams};
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();

        // One synthetic journey touching tile (3,3).
        let mut bm = JourneyBitmap::new();
        let tile = bm.get_tile_mut_or_insert_empty(&crate::journey_bitmap::TileKey::new(3, 3));
        let mut block = crate::journey_bitmap::Block::new();
        block.set_point(0, 0, true);
        tile.set(&crate::journey_bitmap::BlockKey::from_x_y(0, 0), block);
        let journeys = vec![Arc::new(crate::achievement::journey::Journey::from_parts(
            crate::achievement::journey::JourneyId("j1".into()),
            chrono::NaiveDate::from_ymd_opt(2021, 6, 1).unwrap(),
            crate::journey_header::JourneyKind::DefaultKind,
            None,
            None,
            crate::journey_data::JourneyData::Bitmap(bm),
        ))];

        let expected = coverage_from_journeys(&journeys, &regions, &fixture.lookup);

        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let got = coverage(
            ctx.bitmap,
            Some(ctx.first_visited),
            &regions,
            &fixture.lookup,
        );

        let to_map = |v: Vec<Coverage>| -> HashMap<RegionId, Coverage> {
            v.into_iter().map(|c| (c.region_id.clone(), c)).collect()
        };
        assert_eq!(to_map(got), to_map(expected));
    }
}
