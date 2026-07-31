//! Region areas across a four-level hierarchy, read the backend-agnostic way.
//!
//! `region_index.rs` pins ancestor roll-up two levels deep (continent →
//! country) directly against `attribution::attribute`. This goes through the
//! public on-demand reader and all four levels, which is what admin-1/2 will
//! exercise.

pub mod region_fixture;

use std::collections::HashMap;

use geo_data_format::GeoEntityId;
use memolanes_core::{
    achievement::{layer::AchievementLayer, on_demand::region_areas_from_snapshot},
    geo::GeoIndex,
    storage::Storage,
};
use tempdir::TempDir;

use region_fixture::*;

const ALL_IDS: [GeoEntityId; 11] = [
    EU, FR, DE, FR_N, FR_S, DE_W, FR_N_A, FR_N_B, FR_S_A, DE_W_A, UNKNOWN,
];

fn areas(storage: &Storage, geo: &GeoIndex, layer: AchievementLayer) -> HashMap<GeoEntityId, u64> {
    storage
        .with_journey_snapshot(|snap| region_areas_from_snapshot(snap, geo, layer, &ALL_IDS))
        .unwrap()
}

/// Areas are reported in m², rounded from cm² independently per entity, so a
/// parent and the sum of its children can differ by the accumulated rounding.
/// Each value carries at most half a m² of error.
fn assert_rolls_up(areas: &HashMap<GeoEntityId, u64>, parent: GeoEntityId, parts: &[GeoEntityId]) {
    let sum: u64 = parts.iter().map(|id| areas[id]).sum();
    let slack = parts.len() as u64;
    assert!(
        areas[&parent].abs_diff(sum) <= slack,
        "{parent:?} is {} but its parts sum to {sum} (slack {slack})",
        areas[&parent]
    );
}

fn setup(name: &str) -> (TempDir, Storage, GeoIndex) {
    let temp_dir = TempDir::new(name).unwrap();
    let geo_bytes = synthetic_geo_bytes();
    let storage = new_storage(&temp_dir, &geo_bytes);
    let geo = GeoIndex::from_bytes(&geo_bytes).unwrap();
    insert_journeys(&storage);
    (temp_dir, storage, geo)
}

#[test]
fn areas_roll_up_through_four_levels() {
    let (_temp_dir, storage, geo) = setup("region_levels_rollup");
    let areas = areas(&storage, &geo, AchievementLayer::All);

    for city in [FR_N_A, FR_N_B, FR_S_A, DE_W_A] {
        assert!(
            areas.get(&city).is_some_and(|&a| a > 0),
            "{city:?} owns a visited tile, so it must carry area: {areas:?}"
        );
    }

    assert_rolls_up(&areas, FR_N, &[FR_N_A, FR_N_B]);
    assert_rolls_up(&areas, FR_S, &[FR_S_A]);
    assert_rolls_up(&areas, DE_W, &[DE_W_A]);
    assert_rolls_up(&areas, FR, &[FR_N, FR_S]);
    assert_rolls_up(&areas, DE, &[DE_W]);
    assert_rolls_up(&areas, EU, &[FR, DE]);

    assert!(
        !areas.contains_key(&UNKNOWN),
        "an id no entity uses must not be reported as visited"
    );
}

/// The one `Flight` journey lands in Germany and the three `DefaultKind` ones
/// in France, so each layer must see exactly one country's subtree — a
/// per-layer mix-up would otherwise hide behind the `All` totals.
#[test]
fn layers_see_only_their_own_journeys() {
    let (_temp_dir, storage, geo) = setup("region_levels_layers");

    let default = areas(&storage, &geo, AchievementLayer::Default);
    for visited in [EU, FR, FR_N, FR_S, FR_N_A, FR_N_B, FR_S_A] {
        assert!(
            default.contains_key(&visited),
            "Default missing {visited:?}"
        );
    }
    for absent in [DE, DE_W, DE_W_A] {
        assert!(!default.contains_key(&absent), "Default has {absent:?}");
    }

    let flight = areas(&storage, &geo, AchievementLayer::Flight);
    for visited in [EU, DE, DE_W, DE_W_A] {
        assert!(flight.contains_key(&visited), "Flight missing {visited:?}");
    }
    for absent in [FR, FR_N, FR_S, FR_N_A, FR_N_B, FR_S_A] {
        assert!(!flight.contains_key(&absent), "Flight has {absent:?}");
    }
}
