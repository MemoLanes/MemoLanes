//! Refine a coarser entity raster with a finer one.
//!
//! Both operations here are level-agnostic — they take a coarse raster and a
//! fine one and never look at which admin level either is. Today that is
//! countries refined by provinces; an admin-2 level would reuse them unchanged,
//! with the province raster as the coarse side. Only the parenting policy in
//! `entities` is level-specific, because it reads a particular Natural Earth
//! column.
//!
//! The coarse mask is authoritative: a block's leaf is the fine entity only
//! when that entity's parent is the coarse one the block already belongs to.
//! Everything else keeps the coarse entity, so refinement preserves exactly
//! which cells are covered and which coarse entity each cell rolls up to, and a
//! fine polygon that spills across a border is clipped rather than stealing a
//! neighbour's land.
//!
//! Country *areas* do move a little, and that is expected: a `Single(country)`
//! tile split among provinces becomes `Border`, which changes the order
//! `area.rs` adds the f64 cell areas in. Measured against the real sources that
//! shifts 30 of 239 countries by at most 130 m² and every continent by at most
//! 817 m² — against totals of order 10¹²–10¹³ m².

use std::collections::BTreeMap;
use std::time::Instant;

use geo_data_format::{
    tile_index, GeoEntityId, TileMembership, CELLS_PER_TILE, NO_ENTITY, TILE_COUNT, TILE_GRID_WIDTH,
};

use crate::rasterize::{BlockLookup, TileLookup};

/// How many of each fine entity's blocks fall inside each coarse entity's mask,
/// indexed by the fine entity's id: `coverage[fine.0] = coarse id → block
/// count`. Blocks no coarse entity covers are not counted at all.
///
/// A `Vec` rather than a map because the registry allocates entity ids densely
/// from zero, and this is written once per covered cell — order 10^8 times. The
/// inner map stays a `BTreeMap`: `block_majority` breaks ties on the lowest
/// coarse id, which relies on its ascending iteration order.
pub type Coverage = Vec<BTreeMap<GeoEntityId, u64>>;

/// One tile's cells, without materialising a dense buffer for uniform tiles.
enum TileCells<'a> {
    Empty,
    Uniform(GeoEntityId),
    Dense(&'a [u32]),
}

impl TileCells<'_> {
    fn at(&self, cell: usize) -> Option<GeoEntityId> {
        match self {
            TileCells::Empty => None,
            TileCells::Uniform(id) => Some(*id),
            TileCells::Dense(cells) => (cells[cell] != NO_ENTITY).then(|| GeoEntityId(cells[cell])),
        }
    }
}

fn cells_at<'a>(tiles: &TileLookup, blocks: &'a BlockLookup, tx: u16, ty: u16) -> TileCells<'a> {
    match tiles[tile_index(tx, ty)] {
        TileMembership::None => TileCells::Empty,
        TileMembership::Single(id) => TileCells::Uniform(id),
        TileMembership::Border => match blocks.get(&(tx, ty)) {
            Some(cells) => TileCells::Dense(cells.as_slice()),
            None => TileCells::Empty,
        },
    }
}

/// Count, per fine entity, how many of its blocks each coarse mask covers.
/// Parent selection reads this (see `entities::resolve_province_parents`); it
/// needs the full breakdown rather than just the winner, because a province the
/// mask gives no block of at all cannot be shipped under it.
pub fn measure_coverage(
    coarse: (&TileLookup, &BlockLookup),
    fine: (&TileLookup, &BlockLookup),
) -> Coverage {
    let started = Instant::now();
    let mut tally: Coverage = Coverage::new();
    let mut credit = |pid: GeoEntityId, cid: GeoEntityId, blocks: u64| {
        let slot = pid.0 as usize;
        if slot >= tally.len() {
            tally.resize_with(slot + 1, BTreeMap::new);
        }
        *tally[slot].entry(cid).or_default() += blocks;
    };
    for tx in 0..TILE_GRID_WIDTH as u16 {
        for ty in 0..TILE_GRID_WIDTH as u16 {
            let c = cells_at(coarse.0, coarse.1, tx, ty);
            let p = cells_at(fine.0, fine.1, tx, ty);
            match (&c, &p) {
                // Ocean, or land no fine entity is drawn over: nothing to measure.
                (TileCells::Empty, _) | (_, TileCells::Empty) => {}
                // One coarse, one fine entity, whole tile — the common case
                // inland, and it needs no per-cell walk.
                (TileCells::Uniform(cid), TileCells::Uniform(pid)) => {
                    credit(*pid, *cid, CELLS_PER_TILE as u64);
                }
                _ => {
                    for cell in 0..CELLS_PER_TILE {
                        if let (Some(cid), Some(pid)) = (c.at(cell), p.at(cell)) {
                            credit(pid, cid, 1);
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "[geo_rasterizer] measured {} fine entities against the coarse mask in {:.1?}",
        tally
            .iter()
            .filter(|by_coarse| !by_coarse.is_empty())
            .count(),
        started.elapsed()
    );
    tally
}

/// The country covering the most of one province's blocks. Ties break on the
/// lowest country id so the result is deterministic.
pub fn block_majority(by_coarse: &BTreeMap<GeoEntityId, u64>) -> Option<GeoEntityId> {
    by_coarse
        .iter()
        // BTreeMap iterates by ascending country id, and `max_by_key` keeps the
        // LAST maximum, so compare on (count, Reverse(id)).
        .max_by_key(|&(cid, count)| (*count, std::cmp::Reverse(*cid)))
        .map(|(cid, _)| *cid)
}

/// Apply the fine refinement inside the coarse mask.
/// * on its own parent — the fine entity wins
/// * on a different coarse entity — that entity keeps it, spill is clipped
/// * on no coarse entity (ocean) — `None`; a fine polygon cannot create land
/// * no fine cell at all — the coarse entity
pub fn refine_raster(
    coarse: (&TileLookup, &BlockLookup),
    fine: (&TileLookup, &BlockLookup),
    parent_of: &BTreeMap<GeoEntityId, GeoEntityId>,
) -> (TileLookup, BlockLookup) {
    let started = Instant::now();
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    let mut blocks = BlockLookup::new();
    let mut dense: Vec<u32> = vec![NO_ENTITY; CELLS_PER_TILE];

    for tx in 0..TILE_GRID_WIDTH as u16 {
        for ty in 0..TILE_GRID_WIDTH as u16 {
            let idx = tile_index(tx, ty);
            let c = cells_at(coarse.0, coarse.1, tx, ty);
            let p = cells_at(fine.0, fine.1, tx, ty);

            match (&c, &p) {
                (TileCells::Empty, _) => {}
                // No province here: the country classification carries over
                // verbatim. Uniformity still has to be checked for a dense
                // tile, or a border tile that happens to hold one id
                // everywhere would stop collapsing to `Single`.
                (TileCells::Uniform(cid), TileCells::Empty) => {
                    tiles[idx] = TileMembership::Single(*cid);
                }
                (TileCells::Dense(cells), TileCells::Empty) => match uniform_value(cells) {
                    Some(NO_ENTITY) => {}
                    Some(id) => tiles[idx] = TileMembership::Single(GeoEntityId(id)),
                    None => {
                        tiles[idx] = TileMembership::Border;
                        blocks.insert((tx, ty), cells.to_vec());
                    }
                },
                // One country, one province: a single parent lookup decides the
                // whole tile.
                (TileCells::Uniform(cid), TileCells::Uniform(pid)) => {
                    let leaf = if parent_of.get(pid) == Some(cid) {
                        *pid
                    } else {
                        *cid
                    };
                    tiles[idx] = TileMembership::Single(leaf);
                }
                _ => {
                    let mut uniform: Option<Option<GeoEntityId>> = None;
                    let mut is_uniform = true;
                    for (cell, slot) in dense.iter_mut().enumerate() {
                        let leaf = match (c.at(cell), p.at(cell)) {
                            (Some(cid), Some(pid)) if parent_of.get(&pid) == Some(&cid) => {
                                Some(pid)
                            }
                            (coarse_id, _) => coarse_id,
                        };
                        *slot = leaf.map_or(NO_ENTITY, |id| id.0);
                        match uniform {
                            None => uniform = Some(leaf),
                            Some(seen) if seen == leaf => {}
                            Some(_) => is_uniform = false,
                        }
                    }

                    match (is_uniform, uniform.flatten()) {
                        (true, Some(id)) => tiles[idx] = TileMembership::Single(id),
                        (true, None) => {}
                        (false, _) => {
                            tiles[idx] = TileMembership::Border;
                            blocks.insert((tx, ty), dense.clone());
                        }
                    }
                }
            }
        }
    }

    eprintln!(
        "[geo_rasterizer] merged country and province rasters in {:.1?}",
        started.elapsed()
    );
    (tiles, blocks)
}

/// `Some(v)` when every cell holds `v`, `None` when the tile is mixed.
fn uniform_value(cells: &[u32]) -> Option<u32> {
    let first = *cells.first()?;
    cells.iter().all(|c| *c == first).then_some(first)
}
