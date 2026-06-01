//! Quantifies the regime-3 multiplier: a summary-style query that fans out
//! across continent/country/province either (old) rebuilds the merged
//! bitmap + recomputes first_visited per composite, or (new) builds one
//! CoverageCtx and reuses it. Uses test-support synthetic worldview data.

// `arc_with_non_send_sync`: V2 `JourneyBitmap` carries a `RefCell` mipmap
// cache, so `Journey` is not `Sync`. Achievement code uses `Arc<Journey>`
// purely for refcount sharing within a single computation, never across
// threads.
#![allow(clippy::arc_with_non_send_sync)]

use std::sync::Arc;

use chrono::NaiveDate;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use memolanes_core::achievement::coverage::{coverage, coverage_inputs_from_journeys};
use memolanes_core::achievement::geo_entity::GeoEntityKind;
use memolanes_core::achievement::journey::{Journey, JourneyId};
use memolanes_core::achievement::region::NamedRegion;
use memolanes_core::achievement::test_strategies::{synth_worldview, Grid, SynthParams};
use memolanes_core::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey};
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::JourneyKind;

// Intentionally copied from benches/cache.rs — keep both in sync if the
// finalizer changes. The constant is a Murmur3-style finalizer.
fn hash32(x: u32) -> u32 {
    let x = x ^ (x >> 16);
    let x = x.wrapping_mul(0x45d9f3b); // Murmur3-style finalizer
    x ^ (x >> 16)
}

/// Deterministic synthetic journey: sets bits inside the `Grid::default_4x4`
/// region (origin tile (256,256), 4×4 tiles) so journeys actually intersect
/// synth worldview regions and coverage attribution has real work to do.
fn synth_journey(seed: u32) -> Arc<Journey> {
    let h = hash32(seed);
    let grid = Grid::default_4x4();
    let dim = grid.dim as u32;
    let mut bm = JourneyBitmap::new();
    // Place bits in up to 4 distinct tiles within the 4×4 grid (tile indices
    // 0..15) and vary the block position per tile so different journeys cover
    // different sub-areas, giving first_visited meaningful work.
    for k in 0..4u32 {
        let tile_idx = h.wrapping_add(k * 5) % 16;
        let tx = grid.origin.0 + (tile_idx % dim) as u16;
        let ty = grid.origin.1 + (tile_idx / dim) as u16;
        let tile = bm.get_tile_mut_or_insert_empty(&TileKey::new(tx, ty));
        let mut block = Block::new();
        block.set_point(
            (h.wrapping_add(k * 17) % 64) as u8,
            (h.wrapping_add(k * 31) % 64) as u8,
            true,
        );
        tile.set(&BlockKey::from_x_y((k % 4) as u8, 0u8), block);
    }
    let day = 1 + h % 27;
    let month = 1 + h % 12;
    let year = 2018 + (h % 6) as i32;
    Arc::new(Journey::from_parts(
        JourneyId(format!("synth-{seed}")),
        NaiveDate::from_ymd_opt(year, month, day).unwrap(),
        JourneyKind::DefaultKind,
        None,
        None,
        JourneyData::Bitmap(bm),
    ))
}

// Three levels mirror the real summary composite fan-out (the regime-3
// multiplier under test: continent → country → province per query).
fn fanout_kinds() -> [GeoEntityKind; 3] {
    [
        GeoEntityKind::Continent,
        GeoEntityKind::Country,
        GeoEntityKind::Province,
    ]
}

fn bench_summary_fanout(c: &mut Criterion) {
    let fixture = synth_worldview(&SynthParams::plain());
    let regions_by_kind: Vec<(GeoEntityKind, Vec<NamedRegion>)> = fanout_kinds()
        .iter()
        .map(|k| {
            (
                *k,
                fixture.regions_by_kind.get(k).cloned().unwrap_or_default(),
            )
        })
        .collect();

    let mut group = c.benchmark_group("achievement_summary_fanout");
    for n in [100u32, 1000u32] {
        let journeys: Vec<Arc<Journey>> = (1..=n).map(synth_journey).collect();

        // OLD: each kind rebuilds bitmap + recomputes first_visited.
        group.bench_with_input(BenchmarkId::new("old_per_composite", n), &n, |b, _| {
            b.iter(|| {
                for (_, regions) in &regions_by_kind {
                    let cov = memolanes_core::achievement::coverage::coverage_from_journeys(
                        &journeys,
                        regions,
                        &fixture.lookup,
                    );
                    std::hint::black_box(cov.len());
                }
            });
        });

        // NEW: build inputs once, reuse across the fan-out.
        group.bench_with_input(BenchmarkId::new("new_coverage_ctx", n), &n, |b, _| {
            b.iter(|| {
                let owned = coverage_inputs_from_journeys(&journeys, &fixture.lookup);
                let ctx = owned.ctx();
                for (_, regions) in &regions_by_kind {
                    let cov = coverage(
                        ctx.bitmap,
                        Some(ctx.first_visited),
                        regions,
                        &fixture.lookup,
                    );
                    std::hint::black_box(cov.len());
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_summary_fanout);
criterion_main!(benches);
