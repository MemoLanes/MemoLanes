//! Polygon → tile/block classification.
//!
//! Algorithm: per-entity polygon rasterization via even-odd scanline fill
//! at block-grid resolution, followed by per-tile aggregation.
//!
//! Phase 1 (per entity): rasterize each polygon ring at block resolution.
//!   * Edge endpoints are projected to the 65 536 × 65 536 block grid.
//!   * Edge cells are marked via DDA scan-conversion (border_blocks).
//!   * Interior cells are filled via even-odd scanline: for every integer
//!     scan-line y, collect all edge crossings, sort by x, pair them up,
//!     and mark every block (x, y) with x_in ≤ x < x_out as inside.
//!   * The result is, per entity, a `BTreeMap<(tx, ty), TileBlockMask>` —
//!     for each tile that the entity contributes to, a 128×128 bitmap of
//!     which blocks lie inside the polygon, plus a flag indicating whether
//!     any polygon-edge crosses this tile (i.e. this tile is a "border tile"
//!     for this entity).
//!
//! Phase 2: aggregate per tile. Sort entities by bbox-area ascending
//! (smaller-poly-wins). For each tile that any entity contributes to:
//!   * If any contributing entity has tile.has_edge → tile is Border, do a
//!     per-block fill: each block's owner is the smallest entity whose
//!     bitmap has it set.
//!   * Else (all contributing entities mark the tile as fully-inside, no
//!     edges cross) → tile is Single(smallest entity).
//!
//! Smaller-poly-wins ordering matches the previous brute-force impl.
//! Output for the synthetic golden fixture is byte-identical to the
//! previous algorithm.

use std::collections::BTreeMap;
use std::time::Instant;

use geo::algorithm::BoundingRect;
use geo_data_format::{cell_index, tile_index, GeoEntityId, TileMembership, TILE_COUNT};
use geo_types::MultiPolygon;

use crate::entities::EntityModel;
use crate::parse::ParsedFeature;
use crate::projection::{lng_lat_to_block_xy, BLOCK_GRID_SIZE};

const TILE_WIDTH: usize = 128;
const TILES_TOTAL: usize = TILE_COUNT;
const BLOCKS_PER_TILE: usize = TILE_WIDTH * TILE_WIDTH;
const TILE_BITMAP_WORDS: usize = BLOCKS_PER_TILE / 64; // 256 u64s

/// Per-tile membership classifications, indexed by `geo_data_format::tile_index`
/// (x-major), matching the runtime's `BlockKey::index()` convention.
pub type TileLookup = Vec<TileMembership>;
/// Per-(border-)tile block array, 16,384 entries indexed by
/// `geo_data_format::cell_index` (x-major), = the runtime's `BlockKey::index()`.
/// The internal `TileBitmap` is y-major; the transpose happens once, where this
/// vector is emitted (phase 2 border branch).
pub type BlockLookup = BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>>;

/// 128×128 packed bitmap, indexed `byo * TILE_WIDTH + bxo`.
#[derive(Clone)]
struct TileBitmap {
    /// 256 u64s; each bit set ⇒ block is inside the polygon.
    bits: Vec<u64>,
    /// True if at least one polygon edge crosses this tile.
    has_edge: bool,
}

impl TileBitmap {
    fn new() -> Self {
        Self {
            bits: vec![0u64; TILE_BITMAP_WORDS],
            has_edge: false,
        }
    }
    fn set(&mut self, bxo: usize, byo: usize) {
        let i = byo * TILE_WIDTH + bxo;
        self.bits[i >> 6] |= 1u64 << (i & 63);
    }
}

/// Per-entity result of scanline fill.
struct EntityRaster<'a> {
    id: GeoEntityId,
    adm0_a3: &'a str,
    /// Per-tile bitmap of inside blocks. Only tiles the entity touches
    /// (interior or border) have an entry.
    tiles: BTreeMap<(u16, u16), TileBitmap>,
    /// Sort key for smaller-poly-wins.
    bbox_area_blocks: i64,
}

pub fn rasterize(_features: &[ParsedFeature], model: &EntityModel) -> (TileLookup, BlockLookup) {
    let started = Instant::now();

    // Phase 1: per-entity scanline rasterization.
    let mut entity_rasters: Vec<EntityRaster> = Vec::with_capacity(model.entities.len());
    for (i, e) in model.entities.iter().enumerate() {
        let adm0 = e.canonical_code.as_str();
        let geom = match model.geometry_for_country.get(adm0) {
            Some(g) => g,
            None => continue,
        };
        let bbox_lnglat = match geom.bounding_rect() {
            Some(b) => b,
            None => continue,
        };
        let (min_x, min_y) = lng_lat_to_block_xy(bbox_lnglat.min().x, bbox_lnglat.max().y);
        let (max_x, max_y) = lng_lat_to_block_xy(bbox_lnglat.max().x, bbox_lnglat.min().y);
        let bbox_area_blocks =
            (max_x as i64 - min_x as i64).max(1) * (max_y as i64 - min_y as i64).max(1);

        let tiles = rasterize_entity(geom, min_x, min_y, max_x, max_y);
        entity_rasters.push(EntityRaster {
            id: e.id,
            adm0_a3: adm0,
            tiles,
            bbox_area_blocks,
        });
        if (i + 1).is_multiple_of(32) || i + 1 == model.entities.len() {
            eprintln!(
                "[geo_rasterizer] phase 1: rasterized {}/{} entities (elapsed {:.0?})",
                i + 1,
                model.entities.len(),
                started.elapsed()
            );
        }
    }

    // Sort smaller-first (smaller-poly-wins).
    entity_rasters.sort_by(|a, b| {
        a.bbox_area_blocks
            .cmp(&b.bbox_area_blocks)
            .then_with(|| a.adm0_a3.cmp(b.adm0_a3))
    });

    eprintln!(
        "[geo_rasterizer] phase 1 complete in {:.1?} ({} entities)",
        started.elapsed(),
        entity_rasters.len()
    );

    // Phase 2: invert to per-tile candidate lists.
    let mut tile_candidates: BTreeMap<(u16, u16), Vec<usize>> = BTreeMap::new();
    for (idx, e) in entity_rasters.iter().enumerate() {
        for &tile_key in e.tiles.keys() {
            tile_candidates.entry(tile_key).or_default().push(idx);
        }
    }

    let mut tile_lookup: Vec<TileMembership> = vec![TileMembership::None; TILES_TOTAL];
    let mut block_lookup: BlockLookup = BTreeMap::new();

    let phase2_started = Instant::now();
    let total_tiles = tile_candidates.len();
    let mut tile_done = 0usize;

    for ((tx, ty), cand_indices) in &tile_candidates {
        let tile_idx = tile_index(*tx, *ty);

        // Determine if any candidate has an edge in this tile.
        let any_edge = cand_indices
            .iter()
            .any(|&i| entity_rasters[i].tiles[&(*tx, *ty)].has_edge);

        if !any_edge {
            // No edges cross this tile for any candidate. By the polygon
            // invariant, each candidate either fully contains the tile
            // or doesn't touch it at all, so a fully-filled bitmap is
            // the right Single sentinel. (Scanline fill is now correctly
            // bounded; partial bitmaps with no crossing edge would
            // indicate a bug.)
            let winner = cand_indices.iter().find_map(|&i| {
                let bm = &entity_rasters[i].tiles[&(*tx, *ty)];
                if bm.bits.iter().all(|w| *w == u64::MAX) {
                    Some(entity_rasters[i].id)
                } else {
                    None
                }
            });
            if let Some(id) = winner {
                tile_lookup[tile_idx] = TileMembership::Single(id);
            }
            // else: leave as None.
        } else {
            // Border. Per-block fill: smallest candidate with bit set wins.
            let mut blocks: Vec<Option<GeoEntityId>> = vec![None; BLOCKS_PER_TILE];
            // Iterate candidates in sorted (smaller-first) order. Since
            // tile_candidates was built by iterating entity_rasters in
            // sorted order, cand_indices is already ascending → smaller
            // first.
            for &cand_idx in cand_indices {
                let er = &entity_rasters[cand_idx];
                let bm = &er.tiles[&(*tx, *ty)];
                let value = er.id;
                // Walk the bitmap; only set blocks that are still None.
                for word_idx in 0..TILE_BITMAP_WORDS {
                    let mut w = bm.bits[word_idx];
                    if w == 0 {
                        continue;
                    }
                    let base = word_idx * 64;
                    while w != 0 {
                        let b = w.trailing_zeros() as usize;
                        let i = base + b; // y-major bit index in the TileBitmap
                                          // Transpose to x-major on output: bit i is (x=i%128, y=i/128).
                        let out = cell_index((i % TILE_WIDTH) as u8, (i / TILE_WIDTH) as u8);
                        if blocks[out].is_none() {
                            blocks[out] = Some(value);
                        }
                        w &= w - 1;
                    }
                }
            }
            if blocks.iter().any(Option::is_some) {
                tile_lookup[tile_idx] = TileMembership::Border;
                block_lookup.insert((*tx, *ty), blocks);
            }
            // else: edge crossed but no block actually inside (e.g. a thin
            // sliver outside all blocks' centers). Leave as None.
        }

        tile_done += 1;
        if tile_done.is_multiple_of(8192) || tile_done == total_tiles {
            eprintln!(
                "[geo_rasterizer] phase 2: classified {}/{} tiles (phase elapsed {:.0?}, total {:.0?})",
                tile_done,
                total_tiles,
                phase2_started.elapsed(),
                started.elapsed()
            );
        }
    }

    eprintln!(
        "[geo_rasterizer] rasterization complete in {:.1?} ({} border tiles, {} contributing tiles)",
        started.elapsed(),
        block_lookup.len(),
        total_tiles
    );

    (tile_lookup, block_lookup)
}

/// Per-entity scanline rasterization. Produces a `BTreeMap<(tx, ty), TileBitmap>`
/// giving, for each tile the polygon contributes to, a 128×128 bitmap of
/// inside blocks plus a `has_edge` flag.
fn rasterize_entity(
    geom: &MultiPolygon<f64>,
    bbox_min_x: u32,
    bbox_min_y: u32,
    bbox_max_x: u32,
    bbox_max_y: u32,
) -> BTreeMap<(u16, u16), TileBitmap> {
    let mut tiles: BTreeMap<(u16, u16), TileBitmap> = BTreeMap::new();

    // Step 1: scan-convert all edges to mark border blocks AND collect
    // edges in a per-scanline-row data structure for the fill pass.
    //
    // Edge representation for scanline: each non-horizontal edge stored
    // as (y_min, y_max, x_at_y_min, dx_per_dy).
    struct Edge {
        y_min: i64,
        y_max: i64,      // inclusive
        x_at_y_min: f64, // x value at y == y_min (block coords)
        dx_per_dy: f64,
    }
    let mut edges: Vec<Edge> = Vec::new();

    let mut process_ring = |ring: &geo_types::LineString<f64>, edges: &mut Vec<Edge>| {
        let pts = &ring.0;
        if pts.len() < 2 {
            return;
        }
        for window in pts.windows(2) {
            let a = window[0];
            let b = window[1];
            // Project to block-grid floats so DDA and scanline can share.
            let (ax_u, ay_u) = lng_lat_to_block_xy(a.x, a.y);
            let (bx_u, by_u) = lng_lat_to_block_xy(b.x, b.y);
            // Use float block coords for fractional edge intersections.
            // The projection above gave integer (floor) coords; for scanline
            // accuracy we want the un-floored block-space position. Re-do
            // the projection in float.
            let (ax_f, ay_f) = lng_lat_to_block_xy_float(a.x, a.y);
            let (bx_f, by_f) = lng_lat_to_block_xy_float(b.x, b.y);
            // Mark border blocks via DDA on integer coords.
            mark_border_blocks(
                ax_u as i64,
                ay_u as i64,
                bx_u as i64,
                by_u as i64,
                &mut |bx, by| {
                    let tx = (bx / TILE_WIDTH as i64) as u16;
                    let ty = (by / TILE_WIDTH as i64) as u16;
                    let tile = tiles.entry((tx, ty)).or_insert_with(TileBitmap::new);
                    let bxo = (bx as usize) - tx as usize * TILE_WIDTH;
                    let byo = (by as usize) - ty as usize * TILE_WIDTH;
                    tile.set(bxo, byo);
                    tile.has_edge = true;
                },
            );
            // Add edge to scanline list (skip horizontal edges).
            //
            // Scanlines are at integer y + 0.5. An edge from (x0, y_low) to
            // (x1, y_high) (y_low < y_high) contributes to scanline y iff
            // y_low <= y + 0.5 < y_high, i.e. integer y in
            // [ceil(y_low - 0.5), floor(y_high - 0.5 - eps)].
            //
            // The half-open upper bound (y_high exclusive) is the standard
            // top-edge rule that avoids double-counting at shared vertices.
            if (ay_f - by_f).abs() < f64::EPSILON {
                // Horizontal: contributes no scanline crossings.
                continue;
            }
            let (y_low, y_high, x_at_low, dx_per_dy);
            if ay_f < by_f {
                y_low = ay_f;
                y_high = by_f;
                x_at_low = ax_f;
                dx_per_dy = (bx_f - ax_f) / (by_f - ay_f);
            } else {
                y_low = by_f;
                y_high = ay_f;
                x_at_low = bx_f;
                dx_per_dy = (ax_f - bx_f) / (ay_f - by_f);
            }
            // First scanline strictly inside [y_low, y_high) is y where
            // y + 0.5 >= y_low and y + 0.5 < y_high.
            //   y_min = ceil(y_low - 0.5)
            //   y_max = ceil(y_high - 0.5) - 1   (largest y with y+0.5 < y_high)
            let y_min = (y_low - 0.5).ceil() as i64;
            let y_max = (y_high - 0.5).ceil() as i64 - 1;
            if y_max < y_min {
                continue;
            }
            // x at the first scanline center (y_min + 0.5).
            let y0 = y_min as f64 + 0.5;
            let x_at_y0 = x_at_low + dx_per_dy * (y0 - y_low);
            // Clamp y_min/y_max to bbox to limit work.
            if y_min < bbox_min_y as i64 {
                let dy = bbox_min_y as i64 - y_min;
                let new_x = x_at_y0 + dx_per_dy * dy as f64;
                edges.push(Edge {
                    y_min: bbox_min_y as i64,
                    y_max: y_max.min(bbox_max_y as i64),
                    x_at_y_min: new_x,
                    dx_per_dy,
                });
            } else if y_min <= bbox_max_y as i64 {
                edges.push(Edge {
                    y_min,
                    y_max: y_max.min(bbox_max_y as i64),
                    x_at_y_min: x_at_y0,
                    dx_per_dy,
                });
            }
        }
    };

    for poly in &geom.0 {
        process_ring(poly.exterior(), &mut edges);
        for hole in poly.interiors() {
            process_ring(hole, &mut edges);
        }
    }

    // Step 2: scanline fill. For each integer y in [bbox_min_y, bbox_max_y],
    // gather x-intersections of all active edges at scan-line y + 0.5.
    // Sort, pair, fill.
    //
    // Active edge table: bucket edges by y_min for fast activation.
    let bbox_h = (bbox_max_y - bbox_min_y + 1) as usize;
    let mut edge_buckets: Vec<Vec<usize>> = vec![Vec::new(); bbox_h];
    for (i, e) in edges.iter().enumerate() {
        if e.y_min >= bbox_min_y as i64 && e.y_min <= bbox_max_y as i64 {
            edge_buckets[(e.y_min - bbox_min_y as i64) as usize].push(i);
        }
    }

    // Active edges: store edge index + current x.
    struct Active {
        edge_idx: usize,
        cur_x: f64,
        y_max: i64,
    }
    let mut active: Vec<Active> = Vec::new();
    let mut x_buffer: Vec<f64> = Vec::new();

    for y in bbox_min_y as i64..=bbox_max_y as i64 {
        // Activate new edges starting at this y.
        let bucket_idx = (y - bbox_min_y as i64) as usize;
        for &eidx in &edge_buckets[bucket_idx] {
            let e = &edges[eidx];
            active.push(Active {
                edge_idx: eidx,
                cur_x: e.x_at_y_min,
                y_max: e.y_max,
            });
        }
        // Drop expired edges (y > y_max).
        active.retain(|a| a.y_max >= y);

        if active.is_empty() {
            continue;
        }

        // Collect x-intersections.
        x_buffer.clear();
        for a in &active {
            x_buffer.push(a.cur_x);
        }
        x_buffer.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Pair and fill.
        let mut i = 0;
        while i + 1 < x_buffer.len() {
            let x_in = x_buffer[i];
            let x_out = x_buffer[i + 1];
            // Fill blocks bx whose center (bx + 0.5) lies strictly inside
            // (x_in, x_out).
            //   smallest bx with bx + 0.5 > x_in  ⇔  bx > x_in - 0.5
            //                                     ⇔  bx ≥ floor(x_in - 0.5) + 1
            //   largest bx with bx + 0.5 < x_out  ⇔  bx < x_out - 0.5
            //                                     ⇔  bx ≤ ceil(x_out - 0.5) - 1
            let center_lo = (x_in - 0.5).floor() as i64 + 1;
            let center_hi = (x_out - 0.5).ceil() as i64 - 1;
            let bx_start = center_lo.max(bbox_min_x as i64).max(0);
            let bx_end = center_hi.min(bbox_max_x as i64).min(BLOCK_GRID_SIZE - 1);
            if bx_start <= bx_end {
                fill_blocks_row(&mut tiles, bx_start, bx_end, y);
            }
            i += 2;
        }

        // Advance active edges' cur_x by dx_per_dy.
        for a in &mut active {
            a.cur_x += edges[a.edge_idx].dx_per_dy;
        }
    }

    tiles
}

/// Fill blocks in row `y` from `bx_start` to `bx_end` inclusive into the
/// tiles map. Does NOT set has_edge (interior-only fill).
fn fill_blocks_row(
    tiles: &mut BTreeMap<(u16, u16), TileBitmap>,
    bx_start: i64,
    bx_end: i64,
    y: i64,
) {
    if !(0..BLOCK_GRID_SIZE).contains(&y) {
        return;
    }
    let ty = (y / TILE_WIDTH as i64) as u16;
    let byo = (y as usize) - ty as usize * TILE_WIDTH;
    let mut bx = bx_start;
    while bx <= bx_end {
        let tx = (bx / TILE_WIDTH as i64) as u16;
        let tile_x_end = ((tx as i64 + 1) * TILE_WIDTH as i64) - 1;
        let bx_in_tile_end = bx_end.min(tile_x_end);
        let tile = tiles.entry((tx, ty)).or_insert_with(TileBitmap::new);
        let bxo_start = (bx as usize) - tx as usize * TILE_WIDTH;
        let bxo_end = (bx_in_tile_end as usize) - tx as usize * TILE_WIDTH;
        // Set bits [bxo_start..=bxo_end] in row byo.
        let row_bit_start = byo * TILE_WIDTH + bxo_start;
        let row_bit_end = byo * TILE_WIDTH + bxo_end;
        let word_lo = row_bit_start >> 6;
        let word_hi = row_bit_end >> 6;
        if word_lo == word_hi {
            let lo_bit = row_bit_start & 63;
            let hi_bit = row_bit_end & 63;
            let mask = if hi_bit == 63 {
                u64::MAX << lo_bit
            } else {
                ((1u64 << (hi_bit + 1)) - 1) & !((1u64 << lo_bit) - 1)
            };
            tile.bits[word_lo] |= mask;
        } else {
            let lo_bit = row_bit_start & 63;
            tile.bits[word_lo] |= u64::MAX << lo_bit;
            for w in (word_lo + 1)..word_hi {
                tile.bits[w] = u64::MAX;
            }
            let hi_bit = row_bit_end & 63;
            let mask = if hi_bit == 63 {
                u64::MAX
            } else {
                (1u64 << (hi_bit + 1)) - 1
            };
            tile.bits[word_hi] |= mask;
        }
        bx = tile_x_end + 1;
    }
}

/// DDA: walk integer block coords from (ax, ay) to (bx, by), invoking
/// `f(x, y)` for each block touched (including endpoints). Clamps to
/// the [0, BLOCK_GRID_SIZE) range.
fn mark_border_blocks(ax: i64, ay: i64, bx: i64, by: i64, f: &mut impl FnMut(i64, i64)) {
    let dx = bx - ax;
    let dy = by - ay;
    let steps = dx.abs().max(dy.abs()).max(1) as usize;
    let inv = 1.0 / steps as f64;
    let fx_step = dx as f64 * inv;
    let fy_step = dy as f64 * inv;
    let mut fx = ax as f64;
    let mut fy = ay as f64;
    let mut last: Option<(i64, i64)> = None;
    for _ in 0..=steps {
        let x = (fx.round() as i64).clamp(0, BLOCK_GRID_SIZE - 1);
        let y = (fy.round() as i64).clamp(0, BLOCK_GRID_SIZE - 1);
        if Some((x, y)) != last {
            f(x, y);
            last = Some((x, y));
        }
        fx += fx_step;
        fy += fy_step;
    }
}

/// Float-precision projection of (lng, lat) to block coords, mirrors
/// `projection::lng_lat_to_block_xy` but without flooring.
fn lng_lat_to_block_xy_float(lng: f64, lat: f64) -> (f64, f64) {
    use std::f64::consts::PI;
    let n = (BLOCK_GRID_SIZE) as f64;
    let lat_rad = lat.to_radians();
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (
        x.clamp(0.0, n - f64::EPSILON),
        y.clamp(0.0, n - f64::EPSILON),
    )
}
