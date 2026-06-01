//! Shared test infrastructure for the achievement property suite.
//!
//! Contains: synthetic worldview builder (`synth_worldview`), proptest
//! generators (`arb_*`), and the naive reference oracle
//! (`naive_coverage` and friends). All gated behind `#[cfg(test)]` so
//! nothing leaks into production builds.

// `dead_code`: populated incrementally over the plan's tasks.
// `arc_with_non_send_sync`: V2 `JourneyBitmap` carries a `RefCell` mipmap
// cache, so `Journey` is not `Sync`. Achievement code uses `Arc<Journey>`
// purely for refcount sharing within a single computation, never across
// threads.
#![allow(dead_code, clippy::arc_with_non_send_sync)]

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use proptest::prelude::*;

use super::geo_entity::{GeoEntity, GeoEntityId, GeoEntityKind, Worldview};
use super::geo_lookup::{GeoLookupTable, TileMembership};
use super::journey::{Journey, JourneyId};
use super::poi::PoiList;
use super::region::{geo_regions_of_kind, Coverage, NamedRegion, RegionFootprint};
use crate::journey_bitmap::{
    Block, BlockKey, JourneyBitmap, TileKey, BITMAP_WIDTH, MAP_WIDTH, TILE_WIDTH,
};
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;

/// Synthetic worldview tile-coordinate window. Generators draw bitmap
/// bits inside this window so journeys actually intersect synth regions.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub origin: (u16, u16), // tile_x, tile_y of the top-left tile
    pub dim: u8,            // grid is dim×dim tiles
}

impl Grid {
    pub fn default_4x4() -> Self {
        Self {
            origin: (256, 256), // equator / prime-meridian region
            dim: 4,
        }
    }

    /// Grid straddling the 180° meridian. Used to give the bbox proptests
    /// probabilistic coverage of the wrap-aware longitude path
    /// (`LngArc::from_arcs` taking the wrap branch). With `dim = 4` and
    /// `origin.0 = MAP_WIDTH - 2 = 510`, columns map to tile_x values
    /// `[510, 511, 0, 1]` after wrapping.
    pub fn antimeridian_4x4() -> Self {
        Self {
            origin: ((MAP_WIDTH as u16) - 2, 256),
            dim: 4,
        }
    }

    /// Pick the (tile_x, tile_y) of the n-th tile in row-major order.
    /// `tile_x` wraps modulo `MAP_WIDTH` so a grid whose `origin.0 + dim`
    /// crosses the antimeridian still maps onto valid tile coordinates.
    fn nth_tile(&self, n: u8) -> (u16, u16) {
        let dim = self.dim as u16;
        let row = (n as u16) / dim;
        let col = (n as u16) % dim;
        let tx = (self.origin.0 + col) % (MAP_WIDTH as u16);
        (tx, self.origin.1 + row)
    }
}

/// A sparse JourneyBitmap inside `grid`. 0–4 tiles, 1–8 set bits per tile.
pub fn arb_journey_bitmap(grid: Grid) -> impl Strategy<Value = JourneyBitmap> {
    let n_tiles_max = (grid.dim as usize) * (grid.dim as usize);
    let n_tiles_max = n_tiles_max.min(4) as u8;
    (0u8..=n_tiles_max)
        .prop_flat_map(move |n_tiles| {
            // Pick n_tiles distinct tile indices, then for each pick 1–8 (bx,by)
            // bit coordinates inside that tile's blocks.
            let tile_picks =
                prop::sample::subsequence((0..n_tiles_max).collect::<Vec<_>>(), n_tiles as usize);
            let bits_per_tile = prop::collection::vec(
                prop::collection::vec(
                    (0u8..(BITMAP_WIDTH as u8), 0u8..(BITMAP_WIDTH as u8)),
                    1..=8,
                ),
                n_tiles as usize,
            );
            (tile_picks, bits_per_tile)
        })
        .prop_map(move |(tile_indices, bits_per_tile)| {
            let mut bm = JourneyBitmap::new();
            for (&tile_idx, bits) in tile_indices.iter().zip(bits_per_tile.iter()) {
                let (tx, ty) = grid.nth_tile(tile_idx);
                let tile_key = TileKey::new(tx, ty);
                let tile = bm.get_tile_mut_or_insert_empty(&tile_key);
                // Block at (0,0) within the tile is sufficient — bits per
                // tile are sparse and we only need overlap behaviour, not
                // realistic geographic distribution.
                let block_key = BlockKey::from_x_y(0u8, 0u8);
                let mut block = tile.get(&block_key).cloned().unwrap_or_else(Block::new);
                for &(bx, by) in bits {
                    block.set_point(bx, by, true);
                }
                tile.set(&block_key, block);
            }
            bm
        })
}

/// Dates 2020-01-01 .. 2026-12-31 inclusive (≈2557 days).
fn arb_date_2020_2026() -> impl Strategy<Value = NaiveDate> {
    let start = NaiveDate::from_ymd_opt(2020, 1, 1)
        .unwrap()
        .num_days_from_ce();
    let end = NaiveDate::from_ymd_opt(2026, 12, 31)
        .unwrap()
        .num_days_from_ce();
    (start..=end).prop_map(|n| NaiveDate::from_num_days_from_ce_opt(n).unwrap())
}

fn arb_journey_kind() -> impl Strategy<Value = JourneyKind> {
    prop_oneof![Just(JourneyKind::DefaultKind), Just(JourneyKind::Flight),]
}

prop_compose! {
    pub fn arb_journey(grid: Grid)(
        id in "[a-z]{1,8}",
        date in arb_date_2020_2026(),
        kind in arb_journey_kind(),
        bitmap in arb_journey_bitmap(grid),
    ) -> Arc<Journey> {
        Arc::new(Journey::from_parts(
            JourneyId(id),
            date,
            kind,
            None,
            None,
            JourneyData::Bitmap(bitmap),
        ))
    }
}

/// 0–20 journeys, default date range, default grid bitmaps.
pub fn arb_journey_list(grid: Grid) -> impl Strategy<Value = Vec<Arc<Journey>>> {
    prop::collection::vec(arb_journey(grid), 0..=20)
}

/// 3–10 journeys clustered into a 14-day window. The `window_start`
/// is uniformly sampled from 2020-01-01..=2026-12-31; `window_end`
/// can therefore spill up to 13 days into 2027. Used ONLY by streak
/// properties — the default `arb_journey_list` produces meaningful
/// streaks at vanishingly low probability.
pub fn arb_journey_list_clustered(grid: Grid) -> impl Strategy<Value = Vec<Arc<Journey>>> {
    arb_date_2020_2026().prop_flat_map(move |window_start| {
        let window_end = window_start + chrono::Duration::days(13);
        let window_start_ce = window_start.num_days_from_ce();
        let window_end_ce = window_end.num_days_from_ce();
        prop::collection::vec(
            (
                "[a-z]{1,8}",
                (window_start_ce..=window_end_ce)
                    .prop_map(|n| NaiveDate::from_num_days_from_ce_opt(n).unwrap()),
                arb_journey_kind(),
                arb_journey_bitmap(grid),
            )
                .prop_map(|(id, date, kind, bitmap)| {
                    Arc::new(Journey::from_parts(
                        JourneyId(id),
                        date,
                        kind,
                        None,
                        None,
                        JourneyData::Bitmap(bitmap),
                    ))
                }),
            3..=10,
        )
    })
}

#[derive(Debug, Clone)]
pub struct SynthParams {
    pub grid: Grid,
    pub n_continents: u8,
    pub n_countries: u8,
    pub pois: Vec<PoiSpec>,
    pub border_tile: Option<BorderSpec>,
}

impl SynthParams {
    pub fn plain() -> Self {
        Self {
            grid: Grid::default_4x4(),
            n_continents: 1,
            n_countries: 3,
            pois: vec![],
            border_tile: None,
        }
    }

    pub fn with_border_tile() -> Self {
        Self {
            border_tile: Some(BorderSpec {
                grid_offset: (0, 0),
                split: BlockSplit::VerticalHalf,
                country_a: GeoEntityId(101),
                country_b: GeoEntityId(102),
            }),
            ..Self::plain()
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoiSpec {
    pub list_id: String,
    pub item_id: u32,
    pub grid_offset: (u8, u8),
}

#[derive(Debug, Clone)]
pub struct BorderSpec {
    pub grid_offset: (u8, u8),
    pub split: BlockSplit,
    pub country_a: GeoEntityId,
    pub country_b: GeoEntityId,
}

#[derive(Debug, Clone, Copy)]
pub enum BlockSplit {
    /// `block_x < TILE_WIDTH/2` ⇒ country_a, else country_b.
    VerticalHalf,
    /// `block_y < TILE_WIDTH/2` ⇒ country_a, else country_b.
    HorizontalHalf,
}

pub struct SynthFixture {
    pub lookup: GeoLookupTable,
    pub regions_by_kind: HashMap<GeoEntityKind, Vec<NamedRegion>>,
    pub poi_lists: Vec<PoiList>,
    pub grid: Grid,
}

pub fn synth_worldview(params: &SynthParams) -> SynthFixture {
    // 1. Build entities: 1 continent (id=1) + n_countries (ids 100..).
    let mut entities: HashMap<GeoEntityId, GeoEntity> = HashMap::new();
    let continent_id = GeoEntityId(1);
    entities.insert(
        continent_id,
        GeoEntity {
            id: continent_id,
            kind: GeoEntityKind::Continent,
            iso_code: "SYNTH-CONTINENT".into(),
            name_key: "synth.continent.0".into(),
            parent_id: None,
            total_area_m2: 1_000_000_000_000,
        },
    );
    for i in 0..params.n_countries {
        let cid = GeoEntityId(100 + i as u32);
        entities.insert(
            cid,
            GeoEntity {
                id: cid,
                kind: GeoEntityKind::Country,
                iso_code: format!("C{i}"),
                name_key: format!("synth.country.{i}"),
                parent_id: Some(continent_id),
                total_area_m2: 100_000_000_000,
            },
        );
    }
    // border_tile may reference country ids outside the band range.
    // In SynthParams::with_border_tile() the default ids 101/102 collide
    // with band ids 1/2, so or_insert_with is a no-op there; this fallback
    // only fires for custom BorderSpec values that introduce fresh ids.
    if let Some(b) = &params.border_tile {
        for cid in [b.country_a, b.country_b] {
            entities.entry(cid).or_insert_with(|| GeoEntity {
                id: cid,
                kind: GeoEntityKind::Country,
                iso_code: format!("BC{}", cid.0),
                name_key: format!("synth.country.border.{}", cid.0),
                parent_id: Some(continent_id),
                total_area_m2: 50_000_000_000,
            });
        }
    }

    // 2. Build tile_lookup: grid laid out as `country_id = continent_id` for
    //    grid tiles, `Single(continent_id)` membership. (n_continents=1 so
    //    every grid tile is the same continent for now.) Per-country
    //    attribution happens at the block level via the `country_for_tile`
    //    helper used by the oracle. Tiles outside the grid are `None`.
    let mut tile_lookup: Vec<TileMembership> =
        vec![TileMembership::None; (MAP_WIDTH * MAP_WIDTH) as usize];
    let dim = params.grid.dim as u16;
    for row in 0..dim {
        for col in 0..dim {
            let tx = params.grid.origin.0 + col;
            let ty = params.grid.origin.1 + row;
            let idx = (ty as usize) * (MAP_WIDTH as usize) + (tx as usize);
            // Default: attribute the whole tile to a country chosen by tile_x band.
            let country_idx = (col as u8) % params.n_countries;
            let country_id = GeoEntityId(100 + country_idx as u32);
            tile_lookup[idx] = TileMembership::Single(country_id);
        }
    }

    // 3. Border tile override: when set, promote the chosen tile to
    //    TileMembership::Border and populate block_lookup with a
    //    block-level country split.
    let mut block_lookup: HashMap<(u16, u16), Vec<Option<GeoEntityId>>> = HashMap::new();
    if let Some(border) = &params.border_tile {
        let bx = params.grid.origin.0 + border.grid_offset.0 as u16;
        let by = params.grid.origin.1 + border.grid_offset.1 as u16;
        let idx = (by as usize) * (MAP_WIDTH as usize) + (bx as usize);
        tile_lookup[idx] = TileMembership::Border;

        // CANONICAL INDEX FORMULA (matches geo_lookup.rs:65 lookup()):
        //
        //     index = block_y * TILE_WIDTH + block_x
        //
        // Outer loop must be block_y, inner block_x. Do NOT mirror
        // BlockKey::from_x_y (column-major) or import_data.rs:85.
        let mut blocks: Vec<Option<GeoEntityId>> = vec![None; (TILE_WIDTH * TILE_WIDTH) as usize];
        let half = (TILE_WIDTH / 2) as u8;
        for block_y in 0..(TILE_WIDTH as u8) {
            for block_x in 0..(TILE_WIDTH as u8) {
                let owner = match border.split {
                    BlockSplit::VerticalHalf => {
                        if block_x < half {
                            border.country_a
                        } else {
                            border.country_b
                        }
                    }
                    BlockSplit::HorizontalHalf => {
                        if block_y < half {
                            border.country_a
                        } else {
                            border.country_b
                        }
                    }
                };
                let i = (block_y as usize) * (TILE_WIDTH as usize) + (block_x as usize);
                blocks[i] = Some(owner);
            }
        }
        block_lookup.insert((bx, by), blocks);
    }

    // 4. Worldviews.
    let worldviews = vec![Worldview {
        id: "default".into(),
        name_key: "synth.worldview.default".into(),
        description_key: "synth.worldview.default.desc".into(),
    }];

    let lookup =
        GeoLookupTable::new_synthetic(tile_lookup, block_lookup, entities, worldviews.clone());

    let mut regions_by_kind: HashMap<GeoEntityKind, Vec<NamedRegion>> = HashMap::new();
    for kind in [
        GeoEntityKind::Continent,
        GeoEntityKind::Country,
        GeoEntityKind::Province,
        GeoEntityKind::City,
    ] {
        regions_by_kind.insert(kind, geo_regions_of_kind(&lookup, kind));
    }

    SynthFixture {
        lookup,
        regions_by_kind,
        poi_lists: vec![], // populated only when params.pois is non-empty (later)
        grid: params.grid,
    }
}

pub fn arb_synth_params_plain() -> impl Strategy<Value = SynthParams> {
    Just(SynthParams::plain())
}

pub fn arb_synth_params_with_border_tile() -> impl Strategy<Value = SynthParams> {
    Just(SynthParams::with_border_tile())
}

/// Mixed strategy used by `coverage_matches_naive` (§13 DoD).
/// 70% plain / 30% border-tile by proptest's integer weights.
pub fn arb_synth_params_mixed() -> impl Strategy<Value = SynthParams> {
    prop_oneof![
        7 => arb_synth_params_plain(),
        3 => arb_synth_params_with_border_tile(),
    ]
}

/// Reference implementation: union all journey bitmaps, compute total
/// area. Used by oracle property tests as the spec-derived oracle for
/// `total_explored_area_m2`.
pub fn naive_total_area(journeys: &[Arc<Journey>]) -> u64 {
    use crate::journey_area_utils::compute_journey_bitmap_area;
    let mut union = JourneyBitmap::new();
    for j in journeys {
        let bm = j.bitmap();
        union.merge_with_partial_clone(bm);
    }
    compute_journey_bitmap_area(&union, None)
}

// ---------------------------------------------------------------------------
// Naive oracle: naive_coverage
// ---------------------------------------------------------------------------
//
// Structural divergence from coverage.rs is intentional: both must agree on
// results, not procedure. The fast impl builds first_visited incrementally
// via bit-delta tracking and computes area via a single-pass entity partition.
// The naive impl builds the full union first, derives per-region overlays from
// the union, then walks sorted journeys from scratch per region.

fn block_is_empty(block: &Block) -> bool {
    block.is_empty()
}

/// True if the bitmap contains no set bits. (Tiles may be present but contain
/// only empty blocks if `validate` hasn't been called — walk to be safe.)
fn bitmap_is_empty(bm: &JourneyBitmap) -> bool {
    bm.all_tile_keys().all(|key| {
        bm.peek_tile_without_updating_cache(key, |tile_opt| {
            tile_opt.is_none_or(|tile| tile.iter().all(|(_, block)| block_is_empty(block)))
        })
    })
}

/// Build a bitmap containing only the bits of `src` that belong to region
/// `eid` according to `lookup`.  Used for GeoLookup footprints.
fn covered_template_geo(
    src: &JourneyBitmap,
    eid: GeoEntityId,
    lookup: &GeoLookupTable,
) -> JourneyBitmap {
    let mut out = JourneyBitmap::new();
    let tile_keys: Vec<TileKey> = src.all_tile_keys().copied().collect();
    for tile_pos in &tile_keys {
        let matched: Vec<(BlockKey, Block)> = src.peek_tile_without_updating_cache(tile_pos, |t| {
            let mut blocks = Vec::new();
            if let Some(tile) = t {
                for (block_key, block) in tile.iter() {
                    // Lookup is per block-coordinate (the entire block belongs to one entity
                    // in Single tiles; Border tiles dispatch to block-level assignment).
                    if let Some(found) =
                        lookup.lookup(tile_pos.x, tile_pos.y, block_key.x(), block_key.y())
                    {
                        if found == eid {
                            blocks.push((block_key, block.clone()));
                        }
                    }
                }
            }
            blocks
        });
        if !matched.is_empty() {
            let dst_tile = out.get_tile_mut_or_insert_empty(tile_pos);
            for (block_key, block) in matched {
                dst_tile.set(&block_key, block);
            }
        }
    }
    out
}

/// Build a bitmap that is the intersection of `src` with `footprint`.
/// Used for Bitmap (POI) footprints.
fn covered_template_bitmap(src: &JourneyBitmap, footprint: &JourneyBitmap) -> JourneyBitmap {
    let mut out = src.clone();
    out.intersection(footprint);
    out
}

/// Return `(region_overlay) \ running` — bits in `overlay` that are not yet
/// in `running`.  A non-empty result means this journey contributes a new bit
/// to the region.
fn overlay_minus_running(overlay: &JourneyBitmap, running: &JourneyBitmap) -> JourneyBitmap {
    let mut result = overlay.clone();
    result.difference(running);
    result
}

/// Reference implementation of `coverage`.
///
/// Algorithm:
/// 1. Sort by `(date, id)` for deterministic tie-break.
/// 2. Build full union bitmap (all journeys OR-merged).
/// 3. Per region: derive `covered_overlay` = union ∩ footprint.
/// 4. `covered_area_m2` = area of `covered_overlay`.
/// 5. `first_visited` = date of the first sorted journey whose bitmap,
///    restricted to the region's footprint, is non-empty AND adds at least
///    one bit not yet in a per-region running union.
pub fn naive_coverage(
    journeys: &[Arc<Journey>],
    regions: &[NamedRegion],
    lookup: &GeoLookupTable,
) -> Vec<Coverage> {
    use crate::journey_area_utils::compute_journey_bitmap_area;

    // Step 1 — deterministic sort by (date, id).
    let mut sorted: Vec<&Arc<Journey>> = journeys.iter().collect();
    sorted.sort_by(|a, b| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())));

    // Step 2 — full union.
    let mut union = JourneyBitmap::new();
    for j in &sorted {
        union.merge_with_partial_clone(j.bitmap());
    }

    let mut out = Vec::with_capacity(regions.len());

    for region in regions {
        // Step 3 — per-region covered overlay = union ∩ footprint.
        let covered_overlay = match &region.footprint {
            RegionFootprint::GeoLookup(eid) => covered_template_geo(&union, *eid, lookup),
            RegionFootprint::Bitmap(footprint) => covered_template_bitmap(&union, footprint),
        };

        // Step 4 — area.
        let covered_area = compute_journey_bitmap_area(&covered_overlay, None);

        // Step 5 — first_visited: walk sorted journeys; keep a per-region
        // running union and record the date of the first journey that
        // contributes a NEW bit to the region's footprint.
        let first_visited = if bitmap_is_empty(&covered_overlay) {
            // No bits covered at all — no journey can be first_visited.
            None
        } else {
            let mut region_running = JourneyBitmap::new();
            let mut found_date = None;

            for j in &sorted {
                // Restrict this journey's bitmap to the region's footprint.
                let j_in_region = match &region.footprint {
                    RegionFootprint::GeoLookup(eid) => {
                        covered_template_geo(j.bitmap(), *eid, lookup)
                    }
                    // For Bitmap (POI) footprints, "this journey adds a new bit to the
                    // region" is equivalent to "this journey is the first to overlap the
                    // footprint" — first_visited is set at most once, so once region_running
                    // covers any footprint bit, subsequent journeys produce empty
                    // overlay_minus_running.
                    RegionFootprint::Bitmap(footprint) => {
                        covered_template_bitmap(j.bitmap(), footprint)
                    }
                };

                // Check if this journey adds at least one new bit.
                let new_bits = overlay_minus_running(&j_in_region, &region_running);
                if !bitmap_is_empty(&new_bits) {
                    found_date = Some(j.date);
                    break;
                }

                // No new bits this journey — merge its regional contribution into
                // running and continue to the next journey. (Once first_visited is
                // set, the loop has broken already.)
                region_running.merge_with_partial_clone(&j_in_region);
            }

            found_date
        };

        out.push(Coverage {
            region_id: region.id.clone(),
            covered_area_m2: covered_area,
            total_area_m2: region.total_area_m2,
            first_visited,
        });
    }

    out
}

// ---------------------------------------------------------------------------
// Naive oracle: naive_area_by
// ---------------------------------------------------------------------------
//
// Per-bucket total area: groups journeys by bucket key, runs naive_total_area
// on each group. Mirrors area_by_year/month/week/day/quarter (composites.rs:421-498).
// Bits set across multiple buckets ARE counted in each bucket they appear in.

use super::scope::{BucketKey, TimeBucket};

pub fn date_to_bucket_key(d: NaiveDate, bucket: TimeBucket) -> BucketKey {
    match bucket {
        TimeBucket::Day => BucketKey::Day(d),
        TimeBucket::Week => {
            let iso = d.iso_week();
            BucketKey::Week {
                iso_year: iso.year(),
                iso_week: iso.week(),
            }
        }
        TimeBucket::Month => BucketKey::Month {
            year: d.year(),
            month: d.month(),
        },
        TimeBucket::Quarter => BucketKey::Quarter {
            year: d.year(),
            quarter: ((d.month() - 1) / 3) + 1,
        },
        TimeBucket::Year => BucketKey::Year(d.year()),
    }
}

/// Per-bucket total area: for each unique bucket key, union the
/// journeys whose date falls in that bucket and compute their total
/// area independently. Mirrors `area_by_year` etc. exactly
/// (composites.rs:421-498).
pub fn naive_area_by(journeys: &[Arc<Journey>], bucket: TimeBucket) -> Vec<(BucketKey, u64)> {
    let mut buckets: HashMap<BucketKey, Vec<Arc<Journey>>> = HashMap::new();
    for j in journeys {
        let k = date_to_bucket_key(j.date, bucket);
        buckets.entry(k).or_default().push(j.clone());
    }
    buckets
        .into_iter()
        .map(|(k, js)| (k, naive_total_area(&js)))
        .collect()
}

// ---------------------------------------------------------------------------
// Naive oracles: argmax by distance/duration, day-streak
// ---------------------------------------------------------------------------

pub fn naive_argmax_by_distance(journeys: &[Arc<Journey>]) -> Option<Arc<Journey>> {
    journeys
        .iter()
        .filter(|j| j.facts().distance_m.is_some())
        .max_by(|a, b| {
            let da = a.facts().distance_m.unwrap();
            let db = b.facts().distance_m.unwrap();
            da.partial_cmp(&db)
                .unwrap_or(std::cmp::Ordering::Equal)
                // Tie-break: prefer the lexicographically smaller (date, id).
                .then_with(|| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())))
        })
        .cloned()
}

pub fn naive_argmax_by_duration(journeys: &[Arc<Journey>]) -> Option<Arc<Journey>> {
    journeys
        .iter()
        .filter(|j| j.facts().duration_sec.is_some())
        .max_by(|a, b| {
            let da = a.facts().duration_sec.unwrap();
            let db = b.facts().duration_sec.unwrap();
            da.cmp(&db)
                .then_with(|| (a.date, a.id.as_str()).cmp(&(b.date, b.id.as_str())))
        })
        .cloned()
}

/// Longest run of consecutive distinct days. 0 for empty input.
pub fn naive_day_streak(journeys: &[Arc<Journey>]) -> u32 {
    use std::collections::BTreeSet;
    let dates: BTreeSet<NaiveDate> = journeys.iter().map(|j| j.date).collect();
    let mut best: u32 = 0;
    let mut cur: u32 = 0;
    let mut prev: Option<NaiveDate> = None;
    for d in dates {
        cur = match prev {
            Some(p) if d == p + chrono::Duration::days(1) => cur + 1,
            _ => 1,
        };
        if cur > best {
            best = cur;
        }
        prev = Some(d);
    }
    best
}

#[cfg(test)]
mod oracle_self_check {
    use super::super::region::RegionId;
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn one_bit_journey(
        id: &str,
        date: NaiveDate,
        grid: Grid,
        tile_idx: u8,
        bx: u8,
        by: u8,
    ) -> Arc<Journey> {
        let mut bm = JourneyBitmap::new();
        let (tx, ty) = grid.nth_tile(tile_idx);
        let tile = bm.get_tile_mut_or_insert_empty(&TileKey::new(tx, ty));
        let block_key = BlockKey::from_x_y(0u8, 0u8);
        let mut block = tile.get(&block_key).cloned().unwrap_or_else(Block::new);
        block.set_point(bx, by, true);
        tile.set(&block_key, block);
        Arc::new(Journey::from_parts(
            JourneyId(id.into()),
            date,
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Bitmap(bm),
        ))
    }

    /// CASE 1: empty journeys, non-empty regions ⇒ all-zero coverage.
    #[test]
    fn case_1_empty_journeys() {
        let fixture = synth_worldview(&SynthParams::plain());
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let cov = naive_coverage(&[], &regions, &fixture.lookup);
        assert!(cov.iter().all(|c| c.covered_area_m2 == 0));
        assert!(cov.iter().all(|c| c.first_visited.is_none()));
    }

    /// CASE 2: single journey, single set bit ⇒ exactly one region has
    /// non-zero area (the one containing that tile band).
    #[test]
    fn case_2_single_journey_single_region() {
        let params = SynthParams::plain();
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        // Tile index 0 → tile_x band 0 → country index (0 % n_countries) = 0
        // → country id 100.
        let j = one_bit_journey("a", d("2024-01-01"), params.grid, 0, 5, 5);
        let cov = naive_coverage(&[j], &regions, &fixture.lookup);
        let nonzero: Vec<_> = cov.iter().filter(|c| c.covered_area_m2 > 0).collect();
        assert_eq!(nonzero.len(), 1, "exactly one country covered");
        assert_eq!(nonzero[0].first_visited, Some(d("2024-01-01")));
    }

    /// CASE 3: two journeys overlap same region; first_visited = journey 1.
    #[test]
    fn case_3_overlap_same_region_first_visited() {
        let params = SynthParams::plain();
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let j1 = one_bit_journey("a", d("2024-01-01"), params.grid, 0, 5, 5);
        let j2 = one_bit_journey("b", d("2024-06-01"), params.grid, 0, 5, 5); // same bit
        let cov = naive_coverage(&[j2.clone(), j1.clone()], &regions, &fixture.lookup);
        let visited: Vec<_> = cov.iter().filter(|c| c.first_visited.is_some()).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].first_visited, Some(d("2024-01-01")));
    }

    /// CASE 4: same date, two journeys with different ids; first_visited
    /// tie-broken by lexicographically smaller id.
    #[test]
    fn case_4_same_date_id_tie_break() {
        let params = SynthParams::plain();
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        let j_b = one_bit_journey("b", d("2024-01-01"), params.grid, 0, 5, 5);
        let j_a = one_bit_journey("a", d("2024-01-01"), params.grid, 0, 5, 5);
        let cov = naive_coverage(&[j_b, j_a], &regions, &fixture.lookup);
        let visited: Vec<_> = cov.iter().filter(|c| c.first_visited.is_some()).collect();
        assert_eq!(visited.len(), 1);
        assert_eq!(visited[0].first_visited, Some(d("2024-01-01")));
    }

    /// CASE 5: border tile via BlockSplit::VerticalHalf. Bits in the
    /// left half attribute to country_a, right half to country_b.
    #[test]
    fn case_5_border_tile_attribution() {
        let params = SynthParams::with_border_tile();
        let fixture = synth_worldview(&params);
        // Left-half bit (block_x = 0)
        let mut bm_left = JourneyBitmap::new();
        let (tx, ty) = params.grid.nth_tile(0);
        let tile_key = TileKey::new(tx, ty);
        let tile = bm_left.get_tile_mut_or_insert_empty(&tile_key);
        let block_key_left = BlockKey::from_x_y(0u8, 0u8);
        let mut block_left = tile
            .get(&block_key_left)
            .cloned()
            .unwrap_or_else(Block::new);
        block_left.set_point(5, 5, true);
        tile.set(&block_key_left, block_left);
        let j_left = Arc::new(Journey::from_parts(
            JourneyId("L".into()),
            d("2024-01-01"),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Bitmap(bm_left),
        ));
        // Right-half bit (block_x = TILE_WIDTH - 1)
        let mut bm_right = JourneyBitmap::new();
        let tile = bm_right.get_tile_mut_or_insert_empty(&tile_key);
        let block_key_right = BlockKey::from_x_y((TILE_WIDTH as u8) - 1, 0u8);
        let mut block_right = tile
            .get(&block_key_right)
            .cloned()
            .unwrap_or_else(Block::new);
        block_right.set_point(5, 5, true);
        tile.set(&block_key_right, block_right);
        let j_right = Arc::new(Journey::from_parts(
            JourneyId("R".into()),
            d("2024-01-02"),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Bitmap(bm_right),
        ));
        let country_a_region = NamedRegion {
            id: RegionId::GeoEntity(GeoEntityId(101)),
            name_key: "a".into(),
            footprint: RegionFootprint::GeoLookup(GeoEntityId(101)),
            total_area_m2: 1,
        };
        let country_b_region = NamedRegion {
            id: RegionId::GeoEntity(GeoEntityId(102)),
            name_key: "b".into(),
            footprint: RegionFootprint::GeoLookup(GeoEntityId(102)),
            total_area_m2: 1,
        };
        let regions = vec![country_a_region, country_b_region];
        let cov = naive_coverage(&[j_left, j_right], &regions, &fixture.lookup);
        let cov_a = cov
            .iter()
            .find(|c| matches!(c.region_id, RegionId::GeoEntity(GeoEntityId(101))))
            .unwrap();
        let cov_b = cov
            .iter()
            .find(|c| matches!(c.region_id, RegionId::GeoEntity(GeoEntityId(102))))
            .unwrap();
        assert!(
            cov_a.covered_area_m2 > 0,
            "left bit should attribute to country_a"
        );
        assert_eq!(
            cov_a.first_visited,
            Some(d("2024-01-01")),
            "left bit's first_visited (j_left's date)"
        );
        assert!(
            cov_b.covered_area_m2 > 0,
            "right bit should attribute to country_b"
        );
        assert_eq!(
            cov_b.first_visited,
            Some(d("2024-01-02")),
            "right bit's first_visited (j_right's date)"
        );
    }

    /// CASE 6: cross-region bitmap overlap. Journey 1 sets bits in two
    /// regions; journey 2 sets additional bits in only the first.
    /// first_visited per region is independent and is not perturbed by
    /// later journeys extending coverage.
    #[test]
    fn case_6_cross_region_overlap() {
        let params = SynthParams::plain();
        let fixture = synth_worldview(&params);
        let regions = fixture.regions_by_kind[&GeoEntityKind::Country].clone();
        // Journey 1: bits in tile band 0 AND tile band 1.
        let mut bm1 = JourneyBitmap::new();
        for tile_idx in [0u8, 1u8] {
            let (tx, ty) = params.grid.nth_tile(tile_idx);
            let tile = bm1.get_tile_mut_or_insert_empty(&TileKey::new(tx, ty));
            let block_key = BlockKey::from_x_y(0u8, 0u8);
            let mut block = tile.get(&block_key).cloned().unwrap_or_else(Block::new);
            block.set_point(5, 5, true);
            tile.set(&block_key, block);
        }
        let j1 = Arc::new(Journey::from_parts(
            JourneyId("a".into()),
            d("2024-01-01"),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Bitmap(bm1),
        ));
        let mut bm2 = JourneyBitmap::new();
        let (tx, ty) = params.grid.nth_tile(0);
        let tile = bm2.get_tile_mut_or_insert_empty(&TileKey::new(tx, ty));
        let block_key = BlockKey::from_x_y(0u8, 0u8);
        let mut block = tile.get(&block_key).cloned().unwrap_or_else(Block::new);
        block.set_point(7, 7, true);
        tile.set(&block_key, block);
        let j2 = Arc::new(Journey::from_parts(
            JourneyId("b".into()),
            d("2024-06-01"),
            JourneyKind::DefaultKind,
            None,
            None,
            JourneyData::Bitmap(bm2),
        ));
        let cov = naive_coverage(&[j2.clone(), j1.clone()], &regions, &fixture.lookup);
        // Country at band 0 (id 100) and band 1 (id 101 in plain mode):
        let band_0 = cov
            .iter()
            .find(|c| matches!(c.region_id, RegionId::GeoEntity(GeoEntityId(100))))
            .unwrap();
        let band_1 = cov
            .iter()
            .find(|c| matches!(c.region_id, RegionId::GeoEntity(GeoEntityId(101))))
            .unwrap();
        assert_eq!(band_0.first_visited, Some(d("2024-01-01")));
        assert_eq!(band_1.first_visited, Some(d("2024-01-01")));
    }
}
