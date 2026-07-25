//! End-to-end over the Flutter Rust Bridge–facing achievement API (`src/api/achievement.rs`),
//! driven through the global app state exactly as the Flutter layer would.
//!
//! The backend is whatever `achievement::new()` returns, exercised through the
//! public api surface.
//!
//! Journeys are seeded through the real public ingest path (GPS points →
//! finalize), then read back via `get_explored_area*`. The geo initializer and
//! the region entry points are exercised against the real
//! `assets/geo/geo_data_iso.bin` worldview asset (skipped when it is absent).

pub mod test_utils;

use std::collections::HashMap;

use memolanes_core::{
    api::achievement::{
        get_explored_area, get_explored_area_by_layer, init_or_change_geo_data, region_detail,
        region_level_view, region_levels, AchievementLayer, GeoEntityId, RegionKind, Worldview,
    },
    api::api,
    api::import::JourneyInfo,
    gps_processor::RawData,
    import_data,
    journey_header::JourneyKind,
};
use std::fs;
use tempdir::TempDir;

/// Import one GPX track as a finalized journey via the public GPS ingest path.
fn import_gpx_as_journey(path: &str) {
    let (raw, _pre) = import_data::load_gpx(path).unwrap();
    let points: Vec<RawData> = raw
        .into_iter()
        .flatten()
        .filter(|p| p.timestamp_ms.is_some())
        .collect();
    assert!(!points.is_empty(), "no timestamped points in {path}");
    for p in &points {
        api::on_location_update(p.clone(), p.timestamp_ms.unwrap());
    }
    assert!(api::finalize_ongoing_journey().unwrap(), "finalize {path}");
}

fn areas() -> HashMap<AchievementLayer, u64> {
    get_explored_area_by_layer().unwrap()
}

#[test]
fn api_achievement_explored_area_and_region_contract() {
    let dir = TempDir::new("api_achievement").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir(&p).unwrap();
        p.into_os_string().into_string().unwrap()
    };
    api::init(sub("temp"), sub("doc"), sub("support"), sub("cache"));

    use AchievementLayer::*;

    // --- Empty state: every layer is zero, both entry points agree. ---
    let empty = areas();
    assert_eq!(empty.len(), 3);
    for l in [Default, Flight, All] {
        assert_eq!(empty[&l], 0, "empty {l:?}");
        assert_eq!(get_explored_area(l).unwrap(), 0);
    }

    // --- Seed two geographically disjoint journeys (Shanghai, Laojunshan). ---
    import_gpx_as_journey("./tests/data/raw_gps_shanghai.gpx");
    import_gpx_as_journey("./tests/data/raw_gps_laojunshan.gpx");

    // Relabel the second journey to Flight, so the layers are distinct: one
    // Default journey, one Flight journey, `All` the union of the two.
    let mut headers = api::list_all_journeys().unwrap();
    assert_eq!(headers.len(), 2);
    headers.sort_by_key(|h| h.journey_date);
    let flight = &headers[1];
    api::update_journey_metadata(
        &flight.id,
        JourneyInfo {
            journey_date: flight.journey_date,
            start_time: flight.start,
            end_time: flight.end,
            journey_kind: JourneyKind::Flight,
            note: flight.note.clone(),
        },
    )
    .unwrap();

    // --- Total-area API: per-layer values are correct and self-consistent. ---
    let a = areas();
    let (default, flight_area, all) = (a[&Default], a[&Flight], a[&All]);

    // Both entry points return the same numbers under one read.
    for l in [Default, Flight, All] {
        assert_eq!(
            get_explored_area(l).unwrap(),
            a[&l],
            "two APIs disagree {l:?}"
        );
    }

    // Each relabeled journey contributes to exactly its layer.
    assert!(default > 0, "Default area");
    assert!(flight_area > 0, "Flight area");

    // `All` is the union: ≥ each component, and — because the two tracks are on
    // opposite sides of the country (disjoint cells) — equal to the sum up to the
    // area rounding. Each layer rounds its own total independently, so the union
    // may differ from the summed components by one rounding unit either way.
    assert!(all >= default && all >= flight_area, "All is a superset");
    let diff = all as i64 - (default + flight_area) as i64;
    assert!(
        diff.abs() <= 1,
        "disjoint union ≈ sum: all={all} d={default} f={flight_area}"
    );

    // --- Geo-absent contract: before geo bytes are supplied, no geo data is
    // installed, so no regions. ---
    // (`api::init` is a process-global singleton, so this binary keeps a single
    // test; the geo install below continues in the same state.)
    assert!(region_levels().unwrap().is_empty(), "no geo → no levels");
    let view = region_level_view(Default, RegionKind::Country, None).unwrap();
    assert_eq!((view.visited_count, view.region_count), (0, 0));
    assert!(view.entries.is_empty());
    assert_eq!(view.level, RegionKind::Country);
    assert!(
        region_detail(GeoEntityId(1), Default).unwrap().is_none(),
        "no geo → no detail"
    );

    // --- Geo initializer against the real ISO asset. ---
    let asset = test_utils::geo_asset("iso");
    let geo_bytes = fs::read(&asset).unwrap();
    init_or_change_geo_data(Worldview::Iso, &geo_bytes).unwrap();
    // Geo is now installed, so the seeded journeys light up region reads.
    assert!(!region_levels().unwrap().is_empty(), "geo → region levels");
}
