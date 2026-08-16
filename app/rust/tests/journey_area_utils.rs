pub mod test_utils;
use memolanes_core::{import_data, journey_area_utils, journey_bitmap::JourneyBitmap, renderer::*};

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 132.1435370795134;
const END_LAT: f64 = -55.793291910360125;

#[test]
fn test_journey_bitmap_area_m2_rounded() {
    let (bitmap_import, _warnings) =
        import_data::fow::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
    let calculated_area = journey_area_utils::journey_bitmap_area_m2_rounded(&bitmap_import, None);
    assert_eq!(calculated_area, 3035670); // area unit: m^2
}

#[test]
fn partial_update_use_cached_and_recompute_touched_tiles_only() {
    let journey_bitmap = JourneyBitmap::new();
    let mut map_renderer = MapRenderer::new(journey_bitmap);

    map_renderer.update(|bitmap, cb| {
        bitmap.add_line_with_change_callback(START_LNG, START_LAT, END_LNG, END_LAT, cb)
    });
    let _ = map_renderer.get_current_area();

    map_renderer.update(|bitmap, cb| {
        bitmap.add_line_with_change_callback(START_LNG, END_LAT, END_LNG, START_LAT, cb)
    });
    let update_area = map_renderer.get_current_area();

    let mut full_journey_bitmap = JourneyBitmap::new();
    full_journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT);
    full_journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT);
    let full_area = journey_area_utils::journey_bitmap_area_m2_rounded(&full_journey_bitmap, None);

    println!("update_area = {update_area}");
    println!("full_area = {full_area}");
    assert_eq!(
        update_area, full_area,
        "updated area after partial-update must match a full compute"
    );
}

/// Total set-bit count across a bitmap, used only to confirm the disjoint
/// bitmaps built for `area_is_order_independent_under_many_sequential_accumulations`
/// really don't share a block before trusting the area equality below.
fn total_bit_count(bitmap: &JourneyBitmap) -> u32 {
    bitmap
        .all_tile_keys()
        .map(|tile_key| {
            bitmap.peek_tile_without_updating_cache(tile_key, |tile| match tile {
                None => 0,
                Some(tile) => tile.iter().map(|(_, block)| block.count()).sum(),
            })
        })
        .sum()
}

#[test]
fn area_is_order_independent_under_many_sequential_accumulations() {
    // An incremental accumulator adds each journey's cm2 area to a running
    // total instead of recomputing from the whole bitmap. The reported number
    // must be the same either way.
    //
    // Two terms would not prove anything. These 198 disjoint single-line bitmaps
    // span latitudes -80..80 (per-block area varies ~5.7x with cos(lat)) and the
    // whole longitude range. Nothing rounds per call — that is the point: cm2
    // stays integer and rounds once, where a number is reported. Rounding each
    // delta instead would drift by up to half a square metre per merge.
    let lats = [
        -80.0, -64.0, -48.0, -32.0, -16.0, 0.0, 16.0, 32.0, 48.0, 64.0, 80.0,
    ];
    let lngs = [
        -170.0, -150.0, -130.0, -110.0, -90.0, -70.0, -50.0, -30.0, -10.0, 10.0, 30.0, 50.0, 70.0,
        90.0, 110.0, 130.0, 150.0, 170.0,
    ];

    // Each line is short (0.4 deg) relative to the grid spacing (16 deg lat,
    // 20 deg lng), so by construction no two bitmaps touch the same block;
    // the bit-count check below verifies that rather than merely assuming it.
    let mut bitmaps = Vec::new();
    for &lat in &lats {
        for &lng in &lngs {
            let mut bitmap = JourneyBitmap::new();
            bitmap.add_line(lng, lat, lng + 0.4, lat + 0.4);
            bitmaps.push(bitmap);
        }
    }
    assert!(bitmaps.len() >= 40);

    let mut running_total_cm2: i64 = 0;
    let mut naive_rounded_total: u64 = 0;
    let mut running_bit_count: u32 = 0;
    let mut merged = JourneyBitmap::new();
    for bitmap in bitmaps {
        running_total_cm2 += journey_area_utils::journey_bitmap_area_cm2(&bitmap, None);
        naive_rounded_total += journey_area_utils::journey_bitmap_area_m2_rounded(&bitmap, None);
        running_bit_count += total_bit_count(&bitmap);
        merged.merge(bitmap);
    }

    assert_eq!(
        running_bit_count,
        total_bit_count(&merged),
        "input bitmaps must be pairwise disjoint at the block level, or the \
         area equality below would hold for the wrong reason"
    );

    let merged_cm2 = journey_area_utils::journey_bitmap_area_cm2(&merged, None);
    let merged_m2 = journey_area_utils::journey_bitmap_area_m2_rounded(&merged, None);
    assert_eq!(running_total_cm2, merged_cm2);
    assert_eq!(
        journey_area_utils::cm2_to_m2_rounded(running_total_cm2),
        merged_m2
    );
    assert!(merged_m2 > 0);

    // Pins that the sweep still exercises the hazard: rounding per call — the
    // drift integer accumulation exists to rule out — really does disagree here.
    assert_ne!(
        naive_rounded_total, merged_m2,
        "sweep must cover the per-call rounding it exists to rule out"
    );
}

#[test]
fn validate_area_after_map_renderer_replace() {
    let journey_bitmap = JourneyBitmap::new();
    let mut map_renderer = MapRenderer::new(journey_bitmap);

    map_renderer.update(|bitmap, changed| {
        bitmap.add_line_with_change_callback(START_LNG, START_LAT, END_LNG, END_LAT, changed);
    });

    assert!(map_renderer.get_current_area() > 0);

    let (bitmap_import, _warnings) =
        import_data::fow::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();

    map_renderer.replace(bitmap_import.clone());

    let calculated_area = journey_area_utils::journey_bitmap_area_m2_rounded(&bitmap_import, None);
    assert_eq!(map_renderer.get_current_area(), calculated_area); // area unit: m^2
}

/// The property integer cm2 exists to provide: the same coverage delivered as
/// one lump and as k separate merges must produce the identical integer. Under
/// f64 these diverged by 5 ulps at k=2 and 315 ulps at k=500.
#[test]
fn cm2_area_is_identical_whether_merged_at_once_or_in_chunks() {
    let lats = [-72.0, -48.0, -16.0, 0.0, 16.0, 48.0, 72.0];
    let lngs = [-150.0, -90.0, -30.0, 30.0, 90.0, 150.0];

    let mut pieces = Vec::new();
    for &lat in &lats {
        for &lng in &lngs {
            let mut bitmap = JourneyBitmap::new();
            bitmap.add_line(lng, lat, lng + 0.4, lat + 0.4);
            pieces.push(bitmap);
        }
    }
    assert!(pieces.len() >= 40);

    let mut merged = JourneyBitmap::new();
    let mut running_bit_count: u32 = 0;
    for piece in &pieces {
        running_bit_count += total_bit_count(piece);
        merged.merge(piece.clone());
    }
    let whole_cm2 = journey_area_utils::journey_bitmap_area_cm2(&merged, None);
    assert!(whole_cm2 > 0);

    // Summing the pieces is only the same coverage as the merge if no two
    // pieces share a block. The 0.4 deg lines sit on a ≥16 deg lat / 60 deg lng
    // grid so they cannot, but verify it rather than assume it — otherwise a
    // failure here would be ambiguous between overlap and a real accumulator bug.
    assert_eq!(
        running_bit_count,
        total_bit_count(&merged),
        "input bitmaps must be pairwise disjoint at the block level"
    );

    let incremental_cm2: i64 = pieces
        .iter()
        .map(|p| journey_area_utils::journey_bitmap_area_cm2(p, None))
        .sum();

    assert_eq!(
        incremental_cm2, whole_cm2,
        "delta-accumulated cm2 must equal a full rebuild exactly, not approximately"
    );
}

/// The disjoint-pieces test above can't distinguish per-bit quantization
/// from the design's rejected alternative of quantizing a rounded f64
/// per-block total: with disjoint pieces every block appears exactly once on
/// both sides, so `Σ_b round(P·c_b)` is the same sum either way. This test
/// exercises the incremental-merge pattern — `delta.difference(before)` —
/// against pieces that share blocks, so some blocks fill up gradually across
/// several merges. Per-bit quantization (a bit's cm2 area times `bit_count`)
/// distributes over that split by construction, since it never rounds a
/// partial total; quantizing a rounded per-block total would not, because
/// round(a) + round(b) != round(a + b) in general.
#[test]
fn cm2_area_is_identical_when_a_block_fills_across_several_merges() {
    // Several lines crisscrossing the same ~0.003 deg box (well under one
    // block's ~0.0055 deg span), each on a different diagonal so every line
    // both shares bits with the ones before it (at the crossings) and adds
    // bits of its own — the same block's count grows across every merge
    // instead of being fixed by whichever piece touches it first.
    let lat = 22.0;
    let lng = 40.0;
    let side = 0.003;
    let mut pieces = Vec::new();
    for &(x1, y1, x2, y2) in &[
        (0.0, 0.0, 1.0, 1.0),
        (1.0, 0.0, 0.0, 1.0),
        (0.5, 0.0, 0.5, 1.0),
        (0.0, 0.5, 1.0, 0.5),
        (0.0, 0.2, 1.0, 0.8),
        (0.2, 1.0, 0.8, 0.0),
    ] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.add_line(
            lng + x1 * side,
            lat + y1 * side,
            lng + x2 * side,
            lat + y2 * side,
        );
        pieces.push(bitmap);
    }

    // Overlap check: if pieces were block-disjoint, this would equal the
    // merged bit count and the test would collapse into the weaker property
    // above rather than the one it's meant to pin.
    let summed_bit_count: u32 = pieces.iter().map(total_bit_count).sum();

    let mut union = JourneyBitmap::new();
    let mut incremental: i64 = 0;
    for piece in &pieces {
        let mut delta = piece.clone();
        delta.difference(&union);
        incremental += journey_area_utils::journey_bitmap_area_cm2(&delta, None);
        union.merge(piece.clone());
    }

    let merged_bit_count = total_bit_count(&union);
    assert!(
        summed_bit_count > merged_bit_count,
        "pieces must overlap for this test to exercise partial-block quantization"
    );

    let whole_cm2 = journey_area_utils::journey_bitmap_area_cm2(&union, None);
    assert!(whole_cm2 > 0);
    assert_eq!(
        incremental, whole_cm2,
        "delta-accumulated cm2 across overlapping merges must equal a full rebuild exactly"
    );
}

/// JourneyBitmap stores tiles in a std HashMap, so a rebuild sums in
/// per-process-seed order. Integer accumulation makes that irrelevant; f64
/// accumulation did not.
#[test]
fn cm2_area_is_independent_of_insertion_order() {
    let lats = [-60.0, -20.0, 20.0, 60.0];
    let lngs = [-120.0, -40.0, 40.0, 120.0];
    let mut pieces = Vec::new();
    for &lat in &lats {
        for &lng in &lngs {
            let mut bitmap = JourneyBitmap::new();
            bitmap.add_line(lng, lat, lng + 0.4, lat + 0.4);
            pieces.push(bitmap);
        }
    }

    let mut forward = JourneyBitmap::new();
    for piece in pieces.iter() {
        forward.merge(piece.clone());
    }
    let mut backward = JourneyBitmap::new();
    for piece in pieces.iter().rev() {
        backward.merge(piece.clone());
    }

    let forward_cm2 = journey_area_utils::journey_bitmap_area_cm2(&forward, None);
    let backward_cm2 = journey_area_utils::journey_bitmap_area_cm2(&backward, None);
    assert!(forward_cm2 > 0);
    assert_eq!(forward_cm2, backward_cm2);
}

/// `add_line` does not clamp latitude, so a line drawn past the Mercator limit
/// produces a `tile_key.y` outside the in-range 0..512 grid (e.g. lat -89 gives
/// tile_y 642). The row arithmetic that turns a tile/block position into an
/// absolute latitude band must degrade gracefully there rather than overflow.
#[test]
fn extreme_latitudes_do_not_panic_and_produce_finite_area() {
    for &lat in &[-89.0, -86.0, 86.0, 89.0] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.add_line(10.0, lat, 10.4, lat + 0.1);
        let area = journey_area_utils::journey_bitmap_area_m2_rounded(&bitmap, None);
        assert!(
            area < 1_000_000_000_000,
            "a single line's area must stay small, got {area} at lat {lat}"
        );
    }
}
