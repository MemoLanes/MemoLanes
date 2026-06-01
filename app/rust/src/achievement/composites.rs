//! app/rust/src/achievement/composites.rs
//!
//! SINGLE-FILE code-level table of contents for all supported achievements.
//! One named `pub fn` per catalog item, preceded by a
//! `// catalog: #N name MARKER` annotation. The bijection test below parses
//! these annotations and asserts they match CATALOG.
//!
//! Section comments below chunk the file by catalog category.
//!
//! FRB-exposed functions in `api/achievement.rs` call these composites and
//! pack results into the public return types — they do not compute
//! anything themselves.

// V2 `JourneyBitmap` carries a `RefCell` mipmap cache, so `Journey` is not
// `Sync`. Achievement journeys are produced and consumed within a single
// thread; `Arc` is used for cheap refcount sharing inside the pipeline.
#![allow(clippy::arc_with_non_send_sync)]

use std::collections::BTreeSet;
use std::sync::Arc;

use anyhow::Result;
use chrono::{Datelike, NaiveDate};

use super::coverage::{coverage, coverage_from_journeys, CoverageCtx};
use super::geo_entity::{GeoEntityId, GeoEntityKind};
use super::geo_lookup::GeoLookupTable;
use super::journey::Journey;
use super::poi::{PoiId, PoiList, PoiListCompletion};
use super::region::{geo_regions_of_kind, Coverage, NamedRegion, RegionId};
use super::scope::{BucketKey, TimeBucket};
use super::time_bucket::group_by_time;

/// Catalog marker indicating achievement difficulty / tier.
#[derive(Debug, PartialEq, Eq)]
pub enum Marker {
    D1,
    D2,
}

// =====================================================================
// CATALOG — canonical list of supported achievements. The bijection test
// enforces sync with the `// catalog:` annotations on each composite.
// =====================================================================

#[allow(dead_code)] // Used only in #[cfg(test)] bijection_test below.
const CATALOG: &[(&str, &str, Marker)] = &[
    // (id_marker, fn_name, marker)
    ("#1", "visited_continents", Marker::D1),
    ("#2", "visited_countries", Marker::D1),
    ("#3", "visited_provinces", Marker::D1),
    ("#4", "visited_cities", Marker::D2),
    ("#5", "total_explored_area_m2", Marker::D1),
    ("#1-count", "visited_continent_count", Marker::D1),
    ("#2-count", "visited_country_count", Marker::D1),
    ("#3-count", "visited_province_count", Marker::D1),
    ("#1-total", "total_continent_count", Marker::D1),
    ("#2-total", "total_country_count", Marker::D1),
    ("#3-total", "total_province_count", Marker::D1),
    ("#6", "per_continent_coverage", Marker::D1),
    ("#7", "per_country_coverage", Marker::D1),
    (
        "#7-in-continent",
        "countries_visited_in_continent",
        Marker::D1,
    ),
    ("#8", "per_province_coverage", Marker::D1),
    ("#9", "per_city_coverage", Marker::D2),
    ("#10", "first_visited", Marker::D1),
    ("#13", "poi_list_completion", Marker::D2),
    ("#14", "per_poi_visited_dates", Marker::D2),
    ("#16", "longest_journey_by_distance", Marker::D1),
    ("#17", "longest_journey_by_duration", Marker::D1),
    ("#18", "loop_count", Marker::D1),
    ("#19", "total_journey_count", Marker::D1),
    ("#20", "most_regions_touched_in_one_journey", Marker::D1),
    ("#21", "overall_bbox", Marker::D1),
    ("#22", "area_by_year", Marker::D1),
    ("#23", "visited_regions_by_year", Marker::D1),
    ("#24", "area_by_month", Marker::D1),
    ("#25", "area_by_week", Marker::D1),
    ("#26", "area_by_day", Marker::D1),
    ("#27", "best_bucket_by", Marker::D1),
    ("#28", "most_active_bucket", Marker::D1),
    ("#29", "longest_active_day_streak", Marker::D1),
    ("#30", "longest_active_bucket_streak", Marker::D1),
    ("#31", "new_regions_in_year", Marker::D1),
    ("#32", "year_in_review", Marker::D1),
    ("#33", "entity_detail_recursive", Marker::D1),
];

// =====================================================================
// === Geographic coverage (catalog #1–#10) ============================
// =====================================================================

// catalog: #1 visited_continents D1
pub fn visited_continents(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Continent);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
        .into_iter()
        .filter(|c| c.visited())
        .collect()
}

// catalog: #2 visited_countries D1
pub fn visited_countries(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Country);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
        .into_iter()
        .filter(|c| c.visited())
        .collect()
}

// catalog: #3 visited_provinces D1
pub fn visited_provinces(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Province);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
        .into_iter()
        .filter(|c| c.visited())
        .collect()
}

// catalog: #4 visited_cities D2
// D2: returns empty Vec until rasterizer ships city-level cells.
// Tracking issue: TBD when rasterizer work begins.
pub fn visited_cities(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::City);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
        .into_iter()
        .filter(|c| c.visited())
        .collect()
}

// catalog: #5 total_explored_area_m2 D1
pub fn total_explored_area_m2(journeys: &[Arc<Journey>]) -> u64 {
    let mut merged = crate::journey_bitmap::JourneyBitmap::new();
    for j in journeys {
        merged.merge_with_partial_clone(j.bitmap());
    }
    crate::journey_area_utils::compute_journey_bitmap_area(&merged, None)
}

// catalog: #1-count visited_continent_count D1
pub fn visited_continent_count(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> u32 {
    visited_continents(ctx, lookup).len() as u32
}

// catalog: #2-count visited_country_count D1
pub fn visited_country_count(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> u32 {
    visited_countries(ctx, lookup).len() as u32
}

// catalog: #3-count visited_province_count D1
pub fn visited_province_count(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> u32 {
    visited_provinces(ctx, lookup).len() as u32
}

// catalog: #1-total total_continent_count D1
pub fn total_continent_count(lookup: &GeoLookupTable) -> u32 {
    lookup.entities_of_kind(GeoEntityKind::Continent).len() as u32
}

// catalog: #2-total total_country_count D1
pub fn total_country_count(lookup: &GeoLookupTable) -> u32 {
    lookup.entities_of_kind(GeoEntityKind::Country).len() as u32
}

// catalog: #3-total total_province_count D1
pub fn total_province_count(lookup: &GeoLookupTable) -> u32 {
    lookup.entities_of_kind(GeoEntityKind::Province).len() as u32
}

// catalog: #6 per_continent_coverage D1
pub fn per_continent_coverage(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Continent);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
}

// catalog: #7 per_country_coverage D1
pub fn per_country_coverage(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Country);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
}

// catalog: #7-in-continent countries_visited_in_continent D1
pub fn countries_visited_in_continent(
    continent_id: GeoEntityId,
    ctx: &CoverageCtx,
    lookup: &GeoLookupTable,
) -> (u32, u32) {
    let regions: Vec<NamedRegion> = lookup
        .entities_of_kind(GeoEntityKind::Country)
        .into_iter()
        .filter(|e| e.parent_id == Some(continent_id))
        .map(|e| NamedRegion {
            id: RegionId::GeoEntity(e.id),
            name_key: e.name_key.clone(),
            footprint: super::region::RegionFootprint::GeoLookup(e.id),
            total_area_m2: e.total_area_m2,
        })
        .collect();
    let visited = coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
        .iter()
        .filter(|c| c.visited())
        .count() as u32;
    (visited, regions.len() as u32)
}

// catalog: #8 per_province_coverage D1
pub fn per_province_coverage(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::Province);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
}

// catalog: #9 per_city_coverage D2
// D2: empty Vec until rasterizer ships. Tracking issue: TBD.
pub fn per_city_coverage(ctx: &CoverageCtx, lookup: &GeoLookupTable) -> Vec<Coverage> {
    let regions = geo_regions_of_kind(lookup, GeoEntityKind::City);
    coverage(ctx.bitmap, Some(ctx.first_visited), &regions, lookup)
}

// catalog: #10 first_visited D1
pub fn first_visited(
    region_id: RegionId,
    ctx: &CoverageCtx,
    lookup: &GeoLookupTable,
) -> Result<Option<NaiveDate>> {
    let region = match &region_id {
        RegionId::GeoEntity(eid) => {
            let e = lookup
                .get_entity(*eid)
                .ok_or_else(|| anyhow::anyhow!("entity not found: {}", eid.0))?;
            NamedRegion {
                id: region_id.clone(),
                name_key: e.name_key.clone(),
                footprint: super::region::RegionFootprint::GeoLookup(*eid),
                total_area_m2: e.total_area_m2,
            }
        }
        RegionId::Poi { .. } => {
            anyhow::bail!("first_visited does not accept Poi RegionId; use per_poi_visited_dates");
        }
    };
    Ok(
        coverage(ctx.bitmap, Some(ctx.first_visited), &[region], lookup)
            .into_iter()
            .next()
            .and_then(|c| c.first_visited),
    )
}

// =====================================================================
// === POI lists (catalog #13–#15) =====================================
// =====================================================================
// #15 (multiple coexisting POI lists) is satisfied by the registry pattern
// in api/achievement.rs (Task 21+); no dedicated composite fn.
// Task 14 implements #13 and #14 below.

// catalog: #13 poi_list_completion D2
// D2: requires bundled POI data. Returns 0/0 with empty items until data
// lands. Tracking issue: TBD when POI work begins.
pub fn poi_list_completion(
    list: &PoiList,
    journeys: &[Arc<Journey>],
    lookup: &GeoLookupTable,
) -> PoiListCompletion {
    let regions = list.as_named_regions();
    let cov = coverage_from_journeys(journeys, &regions, lookup);
    let visited = cov.iter().filter(|c| c.visited()).count() as u32;
    PoiListCompletion {
        list_id: list.id.clone(),
        total: regions.len() as u32,
        visited,
        items: cov,
    }
}

// catalog: #14 per_poi_visited_dates D2
// D2: returns empty until POI data lands. Tracking issue: TBD.
pub fn per_poi_visited_dates(
    list: &PoiList,
    journeys: &[Arc<Journey>],
    lookup: &GeoLookupTable,
) -> Vec<(PoiId, NaiveDate)> {
    let regions = list.as_named_regions();
    let cov = coverage_from_journeys(journeys, &regions, lookup);
    cov.into_iter()
        .filter_map(|c| match (&c.region_id, c.first_visited) {
            (RegionId::Poi { item_id, .. }, Some(d)) => Some((PoiId(*item_id), d)),
            _ => None,
        })
        .collect()
}

// =====================================================================
// === Per-journey metrics (catalog #16–#21) ===========================
// =====================================================================
// Task 15 implements #16–#21 below.

// catalog: #16 longest_journey_by_distance D1
pub fn longest_journey_by_distance(journeys: &[Arc<Journey>]) -> Option<Arc<Journey>> {
    journeys
        .iter()
        .filter(|j| j.distance_m().is_some())
        .max_by(|a, b| {
            a.distance_m()
                .unwrap_or(0.0)
                .total_cmp(&b.distance_m().unwrap_or(0.0))
        })
        .cloned()
}

// catalog: #17 longest_journey_by_duration D1
pub fn longest_journey_by_duration(journeys: &[Arc<Journey>]) -> Option<Arc<Journey>> {
    journeys
        .iter()
        .filter(|j| j.duration_sec().is_some())
        .max_by_key(|j| j.duration_sec().unwrap_or(0))
        .cloned()
}

// catalog: #18 loop_count D1
pub fn loop_count(journeys: &[Arc<Journey>]) -> u32 {
    journeys.iter().filter(|j| j.is_loop()).count() as u32
}

// catalog: #19 total_journey_count D1
pub fn total_journey_count(journeys: &[Arc<Journey>]) -> u32 {
    journeys.len() as u32
}

// catalog: #20 most_regions_touched_in_one_journey D1
// PERFORMANCE: this composite calls coverage_from_journeys(&[j], regions, ...) per-journey
// — O(N_journeys × N_regions × per_bit_lookup). Acceptable for the typical
// UI use case (called on demand) but do NOT call from inside another tight
// loop.
pub fn most_regions_touched_in_one_journey(
    journeys: &[Arc<Journey>],
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> Option<(Arc<Journey>, u32)> {
    journeys
        .iter()
        .map(|j| {
            let cov = coverage_from_journeys(std::slice::from_ref(j), regions, lookup);
            let count = cov.iter().filter(|c| c.visited()).count() as u32;
            (j.clone(), count)
        })
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
}

// catalog: #21 overall_bbox D1
pub fn overall_bbox(journeys: &[Arc<Journey>]) -> Option<super::journey::BBox> {
    let bboxes: Vec<super::journey::BBox> = journeys.iter().filter_map(|j| j.bbox()).collect();
    if bboxes.is_empty() {
        return None;
    }
    let lat_min = bboxes
        .iter()
        .map(|b| b.lat_min)
        .fold(f64::INFINITY, f64::min);
    let lat_max = bboxes
        .iter()
        .map(|b| b.lat_max)
        .fold(f64::NEG_INFINITY, f64::max);
    let arc = crate::lng_arc::LngArc::from_arcs(bboxes.iter().map(|b| crate::lng_arc::LngArc {
        west: b.lng_west,
        east: b.lng_east,
    }))
    .expect("non-empty bboxes ⇒ Some");
    Some(super::journey::BBox {
        lat_min,
        lat_max,
        lng_west: arc.west,
        lng_east: arc.east,
    })
}

// =====================================================================
// === Time-bucketed (catalog #22–#30) =================================
// =====================================================================
// Task 16 implements #22–#30 below.

// catalog: #22 area_by_year D1
pub fn area_by_year(journeys: &[Arc<Journey>]) -> Vec<(i32, u64)> {
    group_by_time(journeys, TimeBucket::Year)
        .into_iter()
        .map(|(k, js)| {
            let year = match k {
                BucketKey::Year(y) => y,
                _ => unreachable!("group_by_time(Year) returns BucketKey::Year"),
            };
            (year, total_explored_area_m2(&js))
        })
        .collect()
}

// catalog: #23 visited_regions_by_year D1
pub fn visited_regions_by_year(
    journeys: &[Arc<Journey>],
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> Vec<(i32, Vec<RegionId>)> {
    group_by_time(journeys, TimeBucket::Year)
        .into_iter()
        .map(|(k, js)| {
            let year = match k {
                BucketKey::Year(y) => y,
                _ => unreachable!(),
            };
            let cov = coverage_from_journeys(&js, regions, lookup);
            let visited: Vec<RegionId> = cov
                .into_iter()
                .filter(|c| c.visited())
                .map(|c| c.region_id)
                .collect();
            (year, visited)
        })
        .collect()
}

// catalog: #24 area_by_month D1
pub fn area_by_month(journeys: &[Arc<Journey>]) -> Vec<((i32, u32), u64)> {
    group_by_time(journeys, TimeBucket::Month)
        .into_iter()
        .map(|(k, js)| {
            let key = match k {
                BucketKey::Month { year, month } => (year, month),
                _ => unreachable!(),
            };
            (key, total_explored_area_m2(&js))
        })
        .collect()
}

// catalog: #25 area_by_week D1
pub fn area_by_week(journeys: &[Arc<Journey>]) -> Vec<((i32, u32), u64)> {
    group_by_time(journeys, TimeBucket::Week)
        .into_iter()
        .map(|(k, js)| {
            let key = match k {
                BucketKey::Week { iso_year, iso_week } => (iso_year, iso_week),
                _ => unreachable!(),
            };
            (key, total_explored_area_m2(&js))
        })
        .collect()
}

// catalog: #26 area_by_day D1
pub fn area_by_day(journeys: &[Arc<Journey>]) -> Vec<(NaiveDate, u64)> {
    group_by_time(journeys, TimeBucket::Day)
        .into_iter()
        .map(|(k, js)| {
            let date = match k {
                BucketKey::Day(d) => d,
                _ => unreachable!(),
            };
            (date, total_explored_area_m2(&js))
        })
        .collect()
}

// catalog: #27 best_bucket_by D1
pub fn best_bucket_by<M: Ord, F: Fn(&[Arc<Journey>]) -> M>(
    journeys: &[Arc<Journey>],
    bucket: TimeBucket,
    metric: F,
) -> Option<(BucketKey, M)> {
    group_by_time(journeys, bucket)
        .into_iter()
        .map(|(k, js)| {
            let m = metric(&js);
            (k, m)
        })
        .max_by(|a, b| a.1.cmp(&b.1))
}

// catalog: #28 most_active_bucket D1
pub fn most_active_bucket(
    journeys: &[Arc<Journey>],
    bucket: TimeBucket,
) -> Option<(BucketKey, u32)> {
    group_by_time(journeys, bucket)
        .into_iter()
        .map(|(k, js)| (k, js.len() as u32))
        .max_by_key(|(_, n)| *n)
}

// catalog: #29 longest_active_day_streak D1
// IMPLEMENTATION: pure iterator fold over Journey::date — does NOT use
// group_by_time. Listed here only because it's a streak achievement.
pub fn longest_active_day_streak(journeys: &[Arc<Journey>]) -> u32 {
    let dates: BTreeSet<NaiveDate> = journeys.iter().map(|j| j.date).collect();
    let mut max_run: u32 = 0;
    let mut cur_run: u32 = 0;
    let mut prev: Option<NaiveDate> = None;
    for d in &dates {
        cur_run = match prev {
            Some(p) if (*d - p).num_days() == 1 => cur_run + 1,
            _ => 1,
        };
        max_run = max_run.max(cur_run);
        prev = Some(*d);
    }
    max_run
}

// catalog: #30 longest_active_bucket_streak D1
pub fn longest_active_bucket_streak(journeys: &[Arc<Journey>], bucket: TimeBucket) -> u32 {
    let mut buckets: BTreeSet<(i64, i64)> = BTreeSet::new();
    for j in journeys {
        let pos = bucket_to_linear(bucket, j.date);
        buckets.insert(pos);
    }
    let mut max_run: u32 = 0;
    let mut cur_run: u32 = 0;
    let mut prev: Option<(i64, i64)> = None;
    // `buckets` is a BTreeSet — uniqueness guarantees no two equal positions,
    // so linear_diff cannot return 0 here. Non-consecutive returns i64::MAX,
    // which fails the `== 1` check and resets the run.
    for b in &buckets {
        cur_run = match prev {
            Some(p) if linear_diff(p, *b) == 1 => cur_run + 1,
            _ => 1,
        };
        max_run = max_run.max(cur_run);
        prev = Some(*b);
    }
    max_run
}

/// Convert a date+bucket-kind to a linear position so streaks can be
/// computed by integer subtraction.
fn bucket_to_linear(bucket: TimeBucket, d: NaiveDate) -> (i64, i64) {
    match bucket {
        TimeBucket::Day => (0, d.num_days_from_ce() as i64),
        TimeBucket::Week => {
            let iso = d.iso_week();
            (iso.year() as i64, iso.week() as i64)
        }
        TimeBucket::Month => (d.year() as i64, d.month() as i64),
        TimeBucket::Quarter => (d.year() as i64, (((d.month() - 1) / 3) + 1) as i64),
        TimeBucket::Year => (0, d.year() as i64),
    }
}

fn linear_diff(prev: (i64, i64), cur: (i64, i64)) -> i64 {
    // Returns 1 for consecutive bucket positions, i64::MAX otherwise.
    // Invariant: callers must ensure prev != cur (typically via BTreeSet).
    if prev.0 == cur.0 {
        cur.1 - prev.1
    } else if cur.0 - prev.0 == 1 && cur.1 == 1 {
        // Wrap from end-of-prev-major to start-of-cur-major.
        // For Month: prev.minor=12, cur.minor=1. For Quarter: 4→1.
        // For Week: prev.minor=52 or 53, cur.minor=1. We accept all such
        // transitions as consecutive for streak purposes.
        1
    } else {
        i64::MAX
    }
}

// =====================================================================
// === Derived (catalog #31–#33) =======================================
// =====================================================================
// Task 17 implements #31–#33 below and removes #![allow(unused_imports,
// dead_code)]; the bijection test passes after this task.

// catalog: #31 new_regions_in_year D1
pub fn new_regions_in_year(
    year: i32,
    regions: &[NamedRegion],
    journeys: &[Arc<Journey>],
    lookup: &GeoLookupTable,
) -> Vec<RegionId> {
    coverage_from_journeys(journeys, regions, lookup)
        .into_iter()
        .filter(|c| c.visited() && c.first_visited.map(|d| d.year() == year).unwrap_or(false))
        .map(|c| c.region_id)
        .collect()
}

/// Result type for `year_in_review`. Internal — not crossing FRB.
#[derive(Debug, Clone)]
pub struct YearInReview {
    pub year: i32,
    pub area_m2: u64,
    pub journey_count: u32,
    pub new_countries: Vec<RegionId>,
    pub new_provinces: Vec<RegionId>,
}

// catalog: #32 year_in_review D1
pub fn year_in_review(
    year: i32,
    journeys_all: &[Arc<Journey>],
    lookup: &GeoLookupTable,
) -> YearInReview {
    let in_year: Vec<Arc<Journey>> = journeys_all
        .iter()
        .filter(|j| j.date.year() == year)
        .cloned()
        .collect();
    let countries = geo_regions_of_kind(lookup, GeoEntityKind::Country);
    let provinces = geo_regions_of_kind(lookup, GeoEntityKind::Province);
    YearInReview {
        year,
        area_m2: total_explored_area_m2(&in_year),
        journey_count: total_journey_count(&in_year),
        new_countries: new_regions_in_year(year, &countries, journeys_all, lookup),
        new_provinces: new_regions_in_year(year, &provinces, journeys_all, lookup),
    }
}

/// Cached-input sibling of `year_in_review`.
///
/// `bitmap_for_year` must be the merged journey bitmap restricted to the
/// requested year (the FRB caller obtains this via
/// `Storage::get_full_bitmap_in_range`). `first_visited` is the
/// all-time map for the active worldview; "new in year" sets are
/// derived purely by filtering its entries' year.
pub fn year_in_review_from_cached(
    year: i32,
    bitmap_for_year: &crate::journey_bitmap::JourneyBitmap,
    first_visited: &std::collections::HashMap<RegionId, chrono::NaiveDate>,
    journey_count: u32,
    lookup: &GeoLookupTable,
) -> YearInReview {
    let countries: Vec<RegionId> = geo_regions_of_kind(lookup, GeoEntityKind::Country)
        .into_iter()
        .map(|r| r.id)
        .collect();
    let provinces: Vec<RegionId> = geo_regions_of_kind(lookup, GeoEntityKind::Province)
        .into_iter()
        .map(|r| r.id)
        .collect();
    let new_countries: Vec<RegionId> = first_visited
        .iter()
        .filter(|(rid, d)| countries.contains(*rid) && d.year() == year)
        .map(|(rid, _)| rid.clone())
        .collect();
    let new_provinces: Vec<RegionId> = first_visited
        .iter()
        .filter(|(rid, d)| provinces.contains(*rid) && d.year() == year)
        .map(|(rid, _)| rid.clone())
        .collect();
    YearInReview {
        year,
        area_m2: crate::journey_area_utils::compute_journey_bitmap_area(bitmap_for_year, None),
        journey_count,
        new_countries,
        new_provinces,
    }
}

/// Result type for `entity_detail_recursive`. Internal.
#[derive(Debug, Clone)]
pub struct EntityDetailNode {
    pub entity: GeoEntityId,
    pub coverage: Coverage,
    pub children: Vec<EntityDetailNode>,
}

// catalog: #33 entity_detail_recursive D1
// EXCEPTION: this composite walks the geo entity hierarchy via lookup
// methods directly. Reference-data lookups are permitted in composites
// (only Storage IO is forbidden).
pub fn entity_detail_recursive(
    entity_id: GeoEntityId,
    ctx: &CoverageCtx,
    lookup: &GeoLookupTable,
) -> Option<EntityDetailNode> {
    let entity = lookup.get_entity(entity_id)?;
    let region = NamedRegion {
        id: RegionId::GeoEntity(entity_id),
        name_key: entity.name_key.clone(),
        footprint: super::region::RegionFootprint::GeoLookup(entity_id),
        total_area_m2: entity.total_area_m2,
    };
    let cov = coverage(ctx.bitmap, Some(ctx.first_visited), &[region], lookup)
        .into_iter()
        .next()?;

    let child_kind = match entity.kind {
        GeoEntityKind::Continent => Some(GeoEntityKind::Country),
        GeoEntityKind::Country => Some(GeoEntityKind::Province),
        GeoEntityKind::Province => Some(GeoEntityKind::City),
        GeoEntityKind::City => None,
    };
    let children = match child_kind {
        None => Vec::new(),
        Some(k) => lookup
            .entities_of_kind(k)
            .into_iter()
            .filter(|e| e.parent_id == Some(entity_id))
            .filter_map(|e| entity_detail_recursive(e.id, ctx, lookup))
            .collect(),
    };

    Some(EntityDetailNode {
        entity: entity_id,
        coverage: cov,
        children,
    })
}

#[cfg(test)]
mod bijection_test {
    use super::*;

    #[test]
    fn catalog_annotations_match_catalog_const() {
        let src = include_str!("composites.rs");
        let lines: Vec<&str> = src.lines().collect();

        let mut found: Vec<(String, String, Marker)> = Vec::new();

        // Walk lines, collect annotations, and check adjacency.
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("// catalog: ") {
                // Parse "#N name MARKER"
                let mut parts = rest.splitn(3, ' ');
                let id = parts.next().expect("annotation id").to_string();
                let name = parts.next().expect("annotation name").to_string();
                let marker_str = parts.next().expect("annotation marker").trim();
                let marker = match marker_str {
                    "D1" => Marker::D1,
                    "D2" => Marker::D2,
                    other => panic!("unknown marker `{other}`"),
                };

                // Find next non-blank, non-comment line and assert it starts
                // with `pub fn name(` OR `pub fn name<` (allow generics).
                let next_code_line = lines[i + 1..]
                    .iter()
                    .find(|l| {
                        let t = l.trim_start();
                        !t.is_empty() && !t.starts_with("//")
                    })
                    .copied()
                    .unwrap_or("");

                let next_trimmed = next_code_line.trim_start();
                let expect_paren = format!("pub fn {name}(");
                let expect_generic = format!("pub fn {name}<");
                assert!(
                    next_trimmed.starts_with(&expect_paren)
                        || next_trimmed.starts_with(&expect_generic),
                    "Annotation `// catalog: {id} {name} {marker_str}` at line {} is not \
                     immediately followed by `pub fn {name}(` or `pub fn {name}<`.\n\
                     Next code line is: `{next_trimmed}`",
                    i + 1
                );

                found.push((id, name, marker));
            }
        }

        // Every CATALOG row has a matching annotation.
        for (cid, cname, cmarker) in CATALOG {
            let hit = found
                .iter()
                .any(|(id, name, marker)| id == cid && name == cname && marker == cmarker);
            assert!(
                hit,
                "CATALOG row `{cid} {cname} {cmarker:?}` has no matching `// catalog:` annotation in composites.rs"
            );
        }

        // Every annotation has a matching CATALOG row.
        for (fid, fname, fmarker) in &found {
            let hit = CATALOG
                .iter()
                .any(|(cid, cname, cmarker)| cid == fid && cname == fname && cmarker == fmarker);
            assert!(
                hit,
                "Annotation `{fid} {fname} {fmarker:?}` is not in CATALOG"
            );
        }
    }
}

#[cfg(test)]
mod overall_bbox_tests {
    use super::*;
    use crate::achievement::journey::{Journey, JourneyId};
    use crate::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey, MAP_WIDTH};
    use crate::journey_data::JourneyData;
    use crate::journey_header::JourneyKind;
    use chrono::NaiveDate;
    use std::sync::Arc;

    fn one_tile_bitmap_journey(id: &str, tile_x: u16) -> Arc<Journey> {
        let mut bm = JourneyBitmap::new();
        let tile = bm.get_tile_mut_or_insert_empty(&TileKey::new(tile_x, 256));
        let mut block = Block::new();
        block.set_point(0, 0, true);
        tile.set(&BlockKey::from_x_y(0, 0), block);
        Arc::new(Journey::from_parts(
            JourneyId(id.into()),
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            JourneyKind::Flight,
            None,
            None,
            JourneyData::Bitmap(bm),
        ))
    }

    #[test]
    fn overall_bbox_two_journeys_across_antimeridian_is_short() {
        let j1 = one_tile_bitmap_journey("east-of-180", 0);
        let j2 = one_tile_bitmap_journey("west-of-180", (MAP_WIDTH as u16) - 1);
        let bbox = overall_bbox(&[j1, j2]).expect("two journeys → some bbox");
        let arc = crate::lng_arc::LngArc {
            west: bbox.lng_west,
            east: bbox.lng_east,
        };
        assert!(arc.crosses_antimeridian(), "bbox {:?} should wrap", bbox);
        let two_tiles_deg = 360.0 / MAP_WIDTH as f64 * 2.0;
        assert!(
            arc.width_deg() <= two_tiles_deg + 1e-6,
            "bbox span {}° should be at most {}°",
            arc.width_deg(),
            two_tiles_deg,
        );
    }

    #[test]
    fn overall_bbox_empty_input_is_none() {
        assert!(overall_bbox(&[]).is_none());
    }

    use crate::achievement::test_strategies::{arb_journey_list, Grid};
    use crate::lng_arc::arc_contains;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn overall_bbox_contains_all_input_bboxes(
            journeys in arb_journey_list(Grid::default_4x4())
        ) {
            check_overall_bbox_contains_inputs(&journeys)?;
        }

        #[test]
        fn overall_bbox_antimeridian_contains_all_input_bboxes(
            journeys in arb_journey_list(Grid::antimeridian_4x4())
        ) {
            check_overall_bbox_contains_inputs(&journeys)?;
        }
    }

    /// Body of `overall_bbox_contains_all_input_bboxes`, factored so it
    /// can run under both the default grid and the antimeridian-straddling
    /// grid for probabilistic coverage of the wrap branch.
    fn check_overall_bbox_contains_inputs(
        journeys: &[Arc<Journey>],
    ) -> Result<(), proptest::test_runner::TestCaseError> {
        let per_journey: Vec<_> = journeys.iter().filter_map(|j| j.bbox()).collect();
        match overall_bbox(journeys) {
            None => prop_assert!(per_journey.is_empty()),
            Some(merged) => {
                let merged_arc = crate::lng_arc::LngArc {
                    west: merged.lng_west,
                    east: merged.lng_east,
                };
                for b in &per_journey {
                    prop_assert!(merged.lat_min <= b.lat_min);
                    prop_assert!(merged.lat_max >= b.lat_max);
                    // Midpoint of the input arc must lie inside the merged arc.
                    let arc = crate::lng_arc::LngArc {
                        west: b.lng_west,
                        east: b.lng_east,
                    };
                    let mid_offset = arc.width_deg() / 2.0;
                    let mut mid = arc.west + mid_offset;
                    if mid > 180.0 {
                        mid -= 360.0;
                    }
                    prop_assert!(
                        arc_contains(&merged_arc, mid),
                        "merged arc {:?} should contain input mid {} (input arc {:?})",
                        merged_arc,
                        mid,
                        arc
                    );
                }
            }
        }
        Ok(())
    }
}
