//! Integration property-test suite for the public composites in
//! `achievement/composites.rs`. Covers all 37 entries from CATALOG
//! (composites.rs:43-86).
//!
//! Generators and the naive oracle come from
//! `memolanes_core::achievement::test_strategies` (enabled via the
//! `test-support` Cargo feature on the dev-dependency self-reference).
//!
//! Properties are grouped by family:
//!   A. Set-shape:        visited_*, per_*_coverage, countries_visited_in_continent
//!   B. Count / total:    *_count, *_total
//!   C. Area:             total_explored_area_m2, area_by_{year,month,week,day}
//!   D. First-visited:    first_visited, new_regions_in_year, per_poi_visited_dates
//!   E. Argmax:           longest_journey_by_{distance,duration}, best_bucket_by,
//!                        most_active_bucket, most_regions_touched_in_one_journey
//!   F. Streak:           longest_active_day_streak, longest_active_bucket_streak
//!   G. Aggregate:        loop_count, total_journey_count, overall_bbox,
//!                        visited_regions_by_year, year_in_review, entity_detail_recursive
//!   H. POI:              poi_list_completion, per_poi_visited_dates (trivial-empty case)

// `arc_with_non_send_sync`: V2 `JourneyBitmap` carries a `RefCell` mipmap
// cache, so `Journey` is not `Sync`. Achievement code uses `Arc<Journey>`
// purely for refcount sharing within a single computation, never across
// threads.
#![allow(clippy::arc_with_non_send_sync)]

use chrono::Datelike;
use memolanes_core::achievement::composites::*;
use memolanes_core::achievement::coverage::coverage_inputs_from_journeys;
use memolanes_core::achievement::geo_entity::{GeoEntityId, GeoEntityKind};
use memolanes_core::achievement::poi::{PoiId, PoiItem, PoiList};
use memolanes_core::achievement::region::{Coverage, RegionId};
use memolanes_core::achievement::scope::{BucketKey, TimeBucket};
use memolanes_core::achievement::test_strategies::{
    arb_journey_list, arb_journey_list_clustered, arb_synth_params_mixed, date_to_bucket_key,
    naive_area_by, naive_argmax_by_distance, naive_argmax_by_duration, naive_coverage,
    naive_day_streak, naive_total_area, synth_worldview, Grid, SynthParams,
};
use memolanes_core::journey_bitmap::JourneyBitmap;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ============================================================
// A. Set-shape composites
// catalog #1 visited_continents, #2 visited_countries,
// #3 visited_provinces, #4 visited_cities
// #6 per_continent_coverage, #7 per_country_coverage,
// #8 per_province_coverage, #9 per_city_coverage
// #7-in-continent countries_visited_in_continent
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #1 visited_continents — filter of full coverage
    #[test]
    fn visited_continents_matches_naive_filter(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Continent].clone();
        let from_naive: HashSet<RegionId> = naive_coverage(
            &journeys, &regions, &fixture.lookup,
        ).into_iter().filter(|c: &Coverage| c.visited()).map(|c| c.region_id.clone()).collect();
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let from_fast: HashSet<RegionId> = visited_continents(
            &ctx, &fixture.lookup,
        ).into_iter().map(|c| c.region_id.clone()).collect();
        prop_assert_eq!(from_fast, from_naive);
    }

    // #2 visited_countries
    #[test]
    fn visited_countries_matches_naive_filter(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let from_naive: HashSet<RegionId> = naive_coverage(
            &journeys, &regions, &fixture.lookup,
        ).into_iter().filter(|c: &Coverage| c.visited()).map(|c| c.region_id.clone()).collect();
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let from_fast: HashSet<RegionId> = visited_countries(
            &ctx, &fixture.lookup,
        ).into_iter().map(|c| c.region_id.clone()).collect();
        prop_assert_eq!(from_fast, from_naive);
    }

    // #3 visited_provinces — synth has 0 provinces; result is always empty.
    // Documented simplification: synth fixture only has continent + countries.
    #[test]
    fn visited_provinces_is_subset_of_provinces(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = visited_provinces(&ctx, &fixture.lookup);
        // All returned entries must be GeoEntityKind::Province.
        for c in &result {
            if let RegionId::GeoEntity(eid) = &c.region_id {
                let kind = fixture.lookup.entity_kind(*eid).unwrap();
                prop_assert_eq!(kind, GeoEntityKind::Province);
            }
        }
    }

    // #4 visited_cities — D2 stub, always returns empty in synth fixture.
    #[test]
    fn visited_cities_is_empty_in_synth(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = visited_cities(&ctx, &fixture.lookup);
        // Synth fixture has no city entities, so result is always empty.
        prop_assert!(result.is_empty());
    }

    // #6 per_continent_coverage — full coverage list (no filter)
    #[test]
    fn per_continent_coverage_matches_naive(
        params in arb_synth_params_mixed(),
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Continent].clone();
        let naive: HashMap<RegionId, Coverage> = naive_coverage(
            &journeys, &regions, &fixture.lookup,
        ).into_iter().map(|c| (c.region_id.clone(), c)).collect();
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let fast: HashMap<RegionId, Coverage> = per_continent_coverage(
            &ctx, &fixture.lookup,
        ).into_iter().map(|c| (c.region_id.clone(), c)).collect();
        prop_assert_eq!(fast, naive);
    }

    // #7 per_country_coverage — full coverage list
    #[test]
    fn per_country_coverage_matches_naive(
        params in arb_synth_params_mixed(),
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let naive: HashMap<RegionId, Coverage> = naive_coverage(
            &journeys, &regions, &fixture.lookup,
        ).into_iter().map(|c| (c.region_id.clone(), c)).collect();
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let fast: HashMap<RegionId, Coverage> = per_country_coverage(
            &ctx, &fixture.lookup,
        ).into_iter().map(|c| (c.region_id.clone(), c)).collect();
        prop_assert_eq!(fast, naive);
    }

    // #8 per_province_coverage — synth: always empty (no provinces in fixture)
    #[test]
    fn per_province_coverage_is_empty_in_synth(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = per_province_coverage(&ctx, &fixture.lookup);
        prop_assert!(result.is_empty());
    }

    // #9 per_city_coverage — D2 stub; synth: always empty
    #[test]
    fn per_city_coverage_is_empty_in_synth(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = per_city_coverage(&ctx, &fixture.lookup);
        prop_assert!(result.is_empty());
    }

    // #7-in-continent countries_visited_in_continent
    // Property: visited ≤ total, and total == countries with that parent.
    #[test]
    fn countries_visited_in_continent_bounds(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1); // synth has one continent with id=1
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let (visited, total) = countries_visited_in_continent(
            continent_id, &ctx, &fixture.lookup,
        );
        prop_assert!(visited <= total);
        // Total must equal the number of country entities with parent == continent_id
        let expected_total = fixture.lookup.entities_of_kind(GeoEntityKind::Country)
            .into_iter()
            .filter(|e| e.parent_id == Some(continent_id))
            .count() as u32;
        prop_assert_eq!(total, expected_total);
    }

    // #7-in-continent: visited set matches per_country_coverage filtered by parent
    #[test]
    fn countries_visited_in_continent_matches_coverage(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let (visited_count, _total) = countries_visited_in_continent(
            continent_id, &ctx, &fixture.lookup,
        );
        // Cross-check: visited count should match filtering per_country_coverage
        // by those whose entity parent is the continent.
        let per_country = per_country_coverage(&ctx, &fixture.lookup);
        let visited_in_continent = per_country.iter()
            .filter(|c| {
                if let RegionId::GeoEntity(eid) = &c.region_id {
                    let e = fixture.lookup.get_entity(*eid);
                    e.map(|e| e.parent_id == Some(continent_id)).unwrap_or(false)
                        && c.visited()
                } else {
                    false
                }
            })
            .count() as u32;
        prop_assert_eq!(visited_count, visited_in_continent);
    }
}

// ============================================================
// B. Count / total composites
// catalog #1-count visited_continent_count
//         #2-count visited_country_count
//         #3-count visited_province_count
//         #1-total total_continent_count
//         #2-total total_country_count
//         #3-total total_province_count
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // All *_count matches visited_* length
    #[test]
    fn count_matches_visited_set_length(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();

        let vc = visited_continents(&ctx, &fixture.lookup);
        prop_assert_eq!(
            visited_continent_count(&ctx, &fixture.lookup) as usize,
            vc.len(),
        );

        let vco = visited_countries(&ctx, &fixture.lookup);
        prop_assert_eq!(
            visited_country_count(&ctx, &fixture.lookup) as usize,
            vco.len(),
        );

        let vp = visited_provinces(&ctx, &fixture.lookup);
        prop_assert_eq!(
            visited_province_count(&ctx, &fixture.lookup) as usize,
            vp.len(),
        );
    }

    // All *_total matches entity count from lookup
    #[test]
    fn total_count_matches_lookup_entity_count(
        _seed in any::<u8>(),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());

        prop_assert_eq!(
            total_continent_count(&fixture.lookup) as usize,
            fixture.lookup.entities_of_kind(GeoEntityKind::Continent).len(),
        );
        prop_assert_eq!(
            total_country_count(&fixture.lookup) as usize,
            fixture.lookup.entities_of_kind(GeoEntityKind::Country).len(),
        );
        prop_assert_eq!(
            total_province_count(&fixture.lookup) as usize,
            fixture.lookup.entities_of_kind(GeoEntityKind::Province).len(),
        );
    }

    // *_count ≤ *_total always
    #[test]
    fn count_le_total(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        prop_assert!(
            visited_continent_count(&ctx, &fixture.lookup)
                <= total_continent_count(&fixture.lookup)
        );
        prop_assert!(
            visited_country_count(&ctx, &fixture.lookup)
                <= total_country_count(&fixture.lookup)
        );
        prop_assert!(
            visited_province_count(&ctx, &fixture.lookup)
                <= total_province_count(&fixture.lookup)
        );
    }
}

// ============================================================
// C. Area composites
// catalog #5  total_explored_area_m2
//         #22 area_by_year
//         #24 area_by_month
//         #25 area_by_week
//         #26 area_by_day
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #5 total_explored_area_m2 matches naive oracle
    #[test]
    fn total_area_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast = total_explored_area_m2(&journeys);
        let naive = naive_total_area(&journeys);
        prop_assert_eq!(fast, naive);
    }

    // #22 area_by_year matches naive_area_by(Year)
    #[test]
    fn area_by_year_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast: HashMap<i32, u64> = area_by_year(&journeys).into_iter().collect();
        let naive: HashMap<i32, u64> = naive_area_by(&journeys, TimeBucket::Year)
            .into_iter()
            .filter_map(|(k, v)| match k {
                BucketKey::Year(y) => Some((y, v)),
                _ => None,
            })
            .collect();
        prop_assert_eq!(fast, naive);
    }

    // #24 area_by_month matches naive_area_by(Month)
    #[test]
    fn area_by_month_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast: HashMap<(i32, u32), u64> = area_by_month(&journeys).into_iter().collect();
        let naive: HashMap<(i32, u32), u64> = naive_area_by(&journeys, TimeBucket::Month)
            .into_iter()
            .filter_map(|(k, v)| match k {
                BucketKey::Month { year, month } => Some(((year, month), v)),
                _ => None,
            })
            .collect();
        prop_assert_eq!(fast, naive);
    }

    // #25 area_by_week matches naive_area_by(Week)
    #[test]
    fn area_by_week_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast: HashMap<(i32, u32), u64> = area_by_week(&journeys).into_iter().collect();
        let naive: HashMap<(i32, u32), u64> = naive_area_by(&journeys, TimeBucket::Week)
            .into_iter()
            .filter_map(|(k, v)| match k {
                BucketKey::Week { iso_year, iso_week } => Some(((iso_year, iso_week), v)),
                _ => None,
            })
            .collect();
        prop_assert_eq!(fast, naive);
    }

    // #26 area_by_day matches naive_area_by(Day)
    #[test]
    fn area_by_day_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        use chrono::NaiveDate;
        let fast: HashMap<NaiveDate, u64> = area_by_day(&journeys).into_iter().collect();
        let naive: HashMap<NaiveDate, u64> = naive_area_by(&journeys, TimeBucket::Day)
            .into_iter()
            .filter_map(|(k, v)| match k {
                BucketKey::Day(d) => Some((d, v)),
                _ => None,
            })
            .collect();
        prop_assert_eq!(fast, naive);
    }

    // area is a valid u64 (structural — just verify no panic)
    #[test]
    fn total_area_computes(journeys in arb_journey_list(Grid::default_4x4())) {
        let _ = total_explored_area_m2(&journeys);
    }
}

// ============================================================
// D. First-visited composites
// catalog #10 first_visited
//         #31 new_regions_in_year
//         #14 per_poi_visited_dates (trivial empty-fixture case)
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #10 first_visited delegates to coverage for GeoEntity regions
    #[test]
    fn first_visited_matches_coverage(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let naive_cov = naive_coverage(
            &journeys, &regions, &fixture.lookup,
        );
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        for c in &naive_cov {
            if let RegionId::GeoEntity(_eid) = &c.region_id {
                let composite = first_visited(
                    c.region_id.clone(),
                    &ctx,
                    &fixture.lookup,
                ).unwrap();
                prop_assert_eq!(composite, c.first_visited);
            }
        }
    }

    // #31 new_regions_in_year: all returned ids have first_visited == year
    #[test]
    fn new_regions_in_year_first_visited_correct(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let cov = naive_coverage(&journeys, &regions, &fixture.lookup);
        // Pick a year that appears in the data (or a fixed year if empty).
        let year = journeys.first().map(|j| j.date.year()).unwrap_or(2024);
        let new_in_year = new_regions_in_year(
            year, &regions, &journeys, &fixture.lookup,
        );
        // Each returned region must have first_visited in that year.
        for rid in &new_in_year {
            let c = cov.iter().find(|c| &c.region_id == rid).unwrap();
            let fv_year = c.first_visited.map(|d| d.year());
            prop_assert_eq!(fv_year, Some(year));
        }
    }

    // #31 new_regions_in_year: all coverage entries with first_visited == year appear
    #[test]
    fn new_regions_in_year_completeness(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let year = journeys.first().map(|j| j.date.year()).unwrap_or(2024);
        let new_in_year: HashSet<RegionId> = new_regions_in_year(
            year, &regions, &journeys, &fixture.lookup,
        ).into_iter().collect();
        let cov = naive_coverage(&journeys, &regions, &fixture.lookup);
        for c in &cov {
            if c.first_visited.map(|d| d.year() == year).unwrap_or(false) {
                prop_assert!(new_in_year.contains(&c.region_id));
            }
        }
    }

    // #14 per_poi_visited_dates: empty fixture => empty result
    #[test]
    fn per_poi_visited_dates_empty_list(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let empty_list = PoiList {
            id: "empty".into(),
            name_key: "list.empty".into(),
            items: vec![],
        };
        let result = per_poi_visited_dates(
            &empty_list, &journeys, &fixture.lookup,
        );
        prop_assert!(result.is_empty());
    }

    // #14 per_poi_visited_dates: PoiId in result comes from items in the list
    #[test]
    fn per_poi_visited_dates_ids_in_list(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        // Create a small POI list with items that have empty footprints
        // (bitmap journeys won't overlap empty footprint, so result is empty).
        let empty_bm = Arc::new(JourneyBitmap::new());
        let list = PoiList {
            id: "test".into(),
            name_key: "list.test".into(),
            items: vec![
                PoiItem { id: PoiId(1), name_key: "poi.1".into(), footprint: empty_bm.clone(), total_area_m2: 0 },
                PoiItem { id: PoiId(2), name_key: "poi.2".into(), footprint: empty_bm.clone(), total_area_m2: 0 },
            ],
        };
        let result = per_poi_visited_dates(
            &list, &journeys, &fixture.lookup,
        );
        let valid_ids: HashSet<u32> = list.items.iter().map(|i| i.id.0).collect();
        for (poi_id, _date) in &result {
            prop_assert!(valid_ids.contains(&poi_id.0));
        }
    }
}

// ============================================================
// E. Argmax / best-bucket composites
// catalog #16 longest_journey_by_distance
//         #17 longest_journey_by_duration
//         #20 most_regions_touched_in_one_journey
//         #27 best_bucket_by
//         #28 most_active_bucket
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #16 longest_journey_by_distance matches naive oracle
    #[test]
    fn longest_by_distance_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast = longest_journey_by_distance(&journeys);
        let naive = naive_argmax_by_distance(&journeys);
        // Compare by (id, date) since Arc<Journey> doesn't implement Eq.
        prop_assert_eq!(
            fast.as_ref().map(|j| (j.id.as_str().to_string(), j.date)),
            naive.as_ref().map(|j| (j.id.as_str().to_string(), j.date)),
        );
    }

    // #17 longest_journey_by_duration matches naive oracle
    #[test]
    fn longest_by_duration_matches_naive(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast = longest_journey_by_duration(&journeys);
        let naive = naive_argmax_by_duration(&journeys);
        prop_assert_eq!(
            fast.as_ref().map(|j| (j.id.as_str().to_string(), j.date)),
            naive.as_ref().map(|j| (j.id.as_str().to_string(), j.date)),
        );
    }

    // #16 / #17: arb journeys are Bitmap-only → no distance/duration → always None
    #[test]
    fn argmax_none_on_bitmap_journeys(journeys in arb_journey_list(Grid::default_4x4())) {
        // All arb journeys are Bitmap-based, so distance_m / duration_sec are None.
        let dist = longest_journey_by_distance(&journeys);
        let dur = longest_journey_by_duration(&journeys);
        prop_assert!(dist.is_none());
        prop_assert!(dur.is_none());
    }

    // #20 most_regions_touched_in_one_journey: result region count ≤ total regions
    #[test]
    fn most_regions_touched_bounded(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let result = most_regions_touched_in_one_journey(
            &journeys, &regions, &fixture.lookup,
        );
        if let Some((_j, count)) = result {
            prop_assert!(count <= regions.len() as u32);
        }
    }

    // #20 most_regions_touched_in_one_journey: count is max over single-journey coverages
    #[test]
    fn most_regions_touched_is_max(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let result = most_regions_touched_in_one_journey(
            &journeys, &regions, &fixture.lookup,
        );
        if let Some((_j, count)) = &result {
            // Every individual journey's region count must be <= the reported max.
            for j in &journeys {
                let per_j = naive_coverage(
                    std::slice::from_ref(j), &regions, &fixture.lookup,
                );
                let per_j_count = per_j.iter().filter(|c| c.visited()).count() as u32;
                prop_assert!(per_j_count <= *count);
            }
        }
    }

    // #27 best_bucket_by: area is max over all buckets
    #[test]
    fn best_bucket_by_is_max(journeys in arb_journey_list(Grid::default_4x4())) {
        let best = best_bucket_by(&journeys, TimeBucket::Year, total_explored_area_m2);
        if let Some((_key, best_area)) = best {
            let all_areas = area_by_year(&journeys);
            for (_y, area) in &all_areas {
                prop_assert!(*area <= best_area);
            }
        }
    }

    // #28 most_active_bucket: count is max over all buckets
    #[test]
    fn most_active_bucket_is_max(journeys in arb_journey_list(Grid::default_4x4())) {
        for bucket in [TimeBucket::Year, TimeBucket::Month, TimeBucket::Day] {
            let best = most_active_bucket(&journeys, bucket);
            if let Some((_key, best_count)) = best {
                let mut counts: HashMap<BucketKey, u32> = HashMap::new();
                for j in &journeys {
                    let k = date_to_bucket_key(j.date, bucket);
                    *counts.entry(k).or_insert(0) += 1;
                }
                for c in counts.values() {
                    prop_assert!(*c <= best_count);
                }
            }
        }
    }
}

// ============================================================
// F. Streak composites (use arb_journey_list_clustered)
// catalog #29 longest_active_day_streak
//         #30 longest_active_bucket_streak
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #29 longest_active_day_streak matches naive oracle
    #[test]
    fn day_streak_matches_naive(
        journeys in arb_journey_list_clustered(Grid::default_4x4()),
    ) {
        let fast = longest_active_day_streak(&journeys);
        let naive = naive_day_streak(&journeys);
        prop_assert_eq!(fast, naive);
    }

    // #29 day streak ≥ 1 when input is non-empty (clustered always ≥ 3 journeys)
    #[test]
    fn day_streak_at_least_1_when_nonempty(
        journeys in arb_journey_list_clustered(Grid::default_4x4()),
    ) {
        prop_assert!(!journeys.is_empty());
        let streak = longest_active_day_streak(&journeys);
        prop_assert!(streak >= 1);
    }

    // #29 day streak with default (scattered) list: computes without panic
    #[test]
    fn day_streak_computes(journeys in arb_journey_list(Grid::default_4x4())) {
        let _ = longest_active_day_streak(&journeys);
    }

    // #30 longest_active_bucket_streak: result ≥ 1 when non-empty
    #[test]
    fn bucket_streak_at_least_1_when_nonempty(
        journeys in arb_journey_list_clustered(Grid::default_4x4()),
    ) {
        for bucket in [TimeBucket::Day, TimeBucket::Week, TimeBucket::Month, TimeBucket::Year] {
            let streak = longest_active_bucket_streak(&journeys, bucket);
            prop_assert!(streak >= 1);
        }
    }

    // #30 bucket streak with default list: computes without panic
    #[test]
    fn bucket_streak_computes(journeys in arb_journey_list(Grid::default_4x4())) {
        for bucket in [TimeBucket::Day, TimeBucket::Week, TimeBucket::Month, TimeBucket::Year] {
            let _ = longest_active_bucket_streak(&journeys, bucket);
        }
    }

    // #30 day_streak == bucket_streak(Day): both count distinct consecutive days
    #[test]
    fn day_streak_equals_bucket_streak_day(
        journeys in arb_journey_list_clustered(Grid::default_4x4()),
    ) {
        let day_streak = longest_active_day_streak(&journeys);
        let bucket_streak = longest_active_bucket_streak(&journeys, TimeBucket::Day);
        prop_assert_eq!(day_streak, bucket_streak);
    }
}

// ============================================================
// G. Aggregate composites
// catalog #18 loop_count
//         #19 total_journey_count
//         #21 overall_bbox
//         #23 visited_regions_by_year
//         #32 year_in_review
//         #33 entity_detail_recursive
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #18 loop_count == count of is_loop journeys
    // (all arb journeys are Bitmap-based: is_loop is always false)
    #[test]
    fn loop_count_matches_filter(journeys in arb_journey_list(Grid::default_4x4())) {
        let fast = loop_count(&journeys);
        let counted: u32 = journeys.iter().filter(|j| j.is_loop()).count() as u32;
        prop_assert_eq!(fast, counted);
    }

    // #19 total_journey_count == journeys.len()
    #[test]
    fn total_journey_count_matches_len(journeys in arb_journey_list(Grid::default_4x4())) {
        prop_assert_eq!(total_journey_count(&journeys) as usize, journeys.len());
    }

    // #21 overall_bbox: None iff all journeys have no bbox
    #[test]
    fn overall_bbox_none_iff_no_bboxes(journeys in arb_journey_list(Grid::default_4x4())) {
        let result = overall_bbox(&journeys);
        let any_bbox = journeys.iter().any(|j| j.bbox().is_some());
        if !any_bbox {
            prop_assert!(result.is_none());
        }
        if result.is_some() {
            prop_assert!(any_bbox);
        }
    }

    // #21 overall_bbox: empty input => None
    #[test]
    fn overall_bbox_empty_input(_seed in any::<u8>()) {
        let result = overall_bbox(&[]);
        prop_assert!(result.is_none());
    }

    // #23 visited_regions_by_year: per-year visited set matches naive_coverage filtered by year
    #[test]
    fn visited_regions_by_year_matches_naive(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let result = visited_regions_by_year(
            &journeys, &regions, &fixture.lookup,
        );
        for (year, rids) in &result {
            let in_year: Vec<_> = journeys.iter()
                .filter(|j| j.date.year() == *year)
                .cloned()
                .collect();
            let expected: HashSet<RegionId> = naive_coverage(
                &in_year, &regions, &fixture.lookup,
            ).into_iter().filter(|c: &Coverage| c.visited()).map(|c| c.region_id.clone()).collect();
            let actual: HashSet<RegionId> = rids.iter().cloned().collect();
            prop_assert_eq!(expected, actual);
        }
    }

    // #23 visited_regions_by_year: every year in result has journeys in that year
    #[test]
    fn visited_regions_by_year_years_have_journeys(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let result = visited_regions_by_year(
            &journeys, &regions, &fixture.lookup,
        );
        // Each year returned must have at least one journey in that year.
        for (year, _rids) in &result {
            let count = journeys.iter().filter(|j| j.date.year() == *year).count();
            prop_assert!(count > 0);
        }
    }

    // #32 year_in_review: field delegation checks
    #[test]
    fn year_in_review_field_delegation(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let year = journeys.first().map(|j| j.date.year()).unwrap_or(2024);
        let result = year_in_review(year, &journeys, &fixture.lookup);

        prop_assert_eq!(result.year, year);

        // area_m2 == area_by_year for that year
        let area_in_y = area_by_year(&journeys)
            .into_iter()
            .find(|(yy, _)| *yy == year)
            .map(|(_, a)| a)
            .unwrap_or(0);
        prop_assert_eq!(result.area_m2, area_in_y);

        // journey_count == count of journeys in that year
        let count_in_y = journeys.iter().filter(|j| j.date.year() == year).count() as u32;
        prop_assert_eq!(result.journey_count, count_in_y);

        // new_countries: all must have first_visited == year
        let countries = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let all_cov = naive_coverage(&journeys, &countries, &fixture.lookup);
        for rid in &result.new_countries {
            let c = all_cov.iter().find(|c| &c.region_id == rid);
            if let Some(c) = c {
                prop_assert_eq!(c.first_visited.map(|d| d.year()), Some(year));
            }
        }
    }

    // #33 entity_detail_recursive: root entity matches input entity_id
    #[test]
    fn entity_detail_recursive_root_entity(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = entity_detail_recursive(
            continent_id, &ctx, &fixture.lookup,
        );
        prop_assert!(result.is_some());
        let node = result.unwrap();
        prop_assert_eq!(node.entity, continent_id);
    }

    // #33 entity_detail_recursive: coverage region_id matches entity_id
    #[test]
    fn entity_detail_recursive_coverage_region_matches(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let node = entity_detail_recursive(
            continent_id, &ctx, &fixture.lookup,
        ).unwrap();
        prop_assert_eq!(node.coverage.region_id, RegionId::GeoEntity(continent_id));
    }

    // #33 entity_detail_recursive: children are countries (next level) for continent
    #[test]
    fn entity_detail_recursive_children_are_countries(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let node = entity_detail_recursive(
            continent_id, &ctx, &fixture.lookup,
        ).unwrap();
        for child in &node.children {
            if let RegionId::GeoEntity(eid) = &child.coverage.region_id {
                let kind = fixture.lookup.entity_kind(*eid).unwrap();
                prop_assert_eq!(kind, GeoEntityKind::Country);
            } else {
                prop_assert!(false, "child coverage region_id should be GeoEntity");
            }
        }
    }

    // #33 entity_detail_recursive: children count == countries under continent
    #[test]
    fn entity_detail_recursive_children_count(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let continent_id = GeoEntityId(1);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let node = entity_detail_recursive(
            continent_id, &ctx, &fixture.lookup,
        ).unwrap();
        let expected_children = fixture.lookup.entities_of_kind(GeoEntityKind::Country)
            .into_iter()
            .filter(|e| e.parent_id == Some(continent_id))
            .count();
        prop_assert_eq!(node.children.len(), expected_children);
    }

    // #33 entity_detail_recursive: None for non-existent entity
    #[test]
    fn entity_detail_recursive_none_for_unknown(
        journeys in arb_journey_list(Grid::default_4x4()),
    ) {
        let fixture = synth_worldview(&SynthParams::plain());
        let bogus_id = GeoEntityId(9999);
        let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
        let ctx = owned.ctx();
        let result = entity_detail_recursive(
            bogus_id, &ctx, &fixture.lookup,
        );
        prop_assert!(result.is_none());
    }
}

// ============================================================
// H. POI composites
// catalog #13 poi_list_completion
// ============================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    // #13 poi_list_completion: empty list => visited=0, total=0
    #[test]
    fn poi_list_completion_empty_list(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let empty_list = PoiList {
            id: "empty".into(),
            name_key: "list.empty".into(),
            items: vec![],
        };
        let result = poi_list_completion(
            &empty_list, &journeys, &fixture.lookup,
        );
        prop_assert_eq!(result.total, 0u32);
        prop_assert_eq!(result.visited, 0u32);
        prop_assert!(result.items.is_empty());
    }

    // #13 poi_list_completion: visited ≤ total always
    #[test]
    fn poi_list_completion_visited_le_total(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let empty_bm = Arc::new(JourneyBitmap::new());
        let list = PoiList {
            id: "test".into(),
            name_key: "list.test".into(),
            items: vec![
                PoiItem { id: PoiId(10), name_key: "poi.10".into(), footprint: empty_bm.clone(), total_area_m2: 100 },
                PoiItem { id: PoiId(20), name_key: "poi.20".into(), footprint: empty_bm.clone(), total_area_m2: 200 },
                PoiItem { id: PoiId(30), name_key: "poi.30".into(), footprint: empty_bm.clone(), total_area_m2: 300 },
            ],
        };
        let result = poi_list_completion(
            &list, &journeys, &fixture.lookup,
        );
        prop_assert!(result.visited <= result.total);
        prop_assert_eq!(result.total, list.items.len() as u32);
    }

    // #13 poi_list_completion: items length == list.items.len() and list_id correct
    #[test]
    fn poi_list_completion_items_len(journeys in arb_journey_list(Grid::default_4x4())) {
        let fixture = synth_worldview(&SynthParams::plain());
        let empty_bm = Arc::new(JourneyBitmap::new());
        let list = PoiList {
            id: "t".into(),
            name_key: "l.t".into(),
            items: vec![
                PoiItem { id: PoiId(1), name_key: "p.1".into(), footprint: empty_bm.clone(), total_area_m2: 0 },
                PoiItem { id: PoiId(2), name_key: "p.2".into(), footprint: empty_bm.clone(), total_area_m2: 0 },
            ],
        };
        let result = poi_list_completion(
            &list, &journeys, &fixture.lookup,
        );
        prop_assert_eq!(result.items.len(), list.items.len());
        prop_assert_eq!(result.list_id, list.id);
    }
}
