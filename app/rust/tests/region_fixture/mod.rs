//! A synthetic four-level worldview (continent → country → province → city).
//!
//! Admin-1/2 data does not ship yet, so the levels below country are only
//! reachable through a fixture. Shared by the backend-agnostic region tests and
//! the backend-specific ones.

// Each test binary that includes this module uses a subset of it.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;

use chrono::NaiveDate;
use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview, TILE_COUNT,
};
use memolanes_core::{
    achievement::{layer::AchievementLayer, on_demand::region_areas_from_snapshot},
    geo::GeoLookup,
    journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    storage::Storage,
};
use tempdir::TempDir;

pub const EU: GeoEntityId = GeoEntityId(1);
pub const FR: GeoEntityId = GeoEntityId(2);
pub const DE: GeoEntityId = GeoEntityId(3);
pub const FR_N: GeoEntityId = GeoEntityId(4);
pub const FR_S: GeoEntityId = GeoEntityId(5);
pub const DE_W: GeoEntityId = GeoEntityId(6);
pub const FR_N_A: GeoEntityId = GeoEntityId(7);
pub const FR_N_B: GeoEntityId = GeoEntityId(8);
pub const FR_S_A: GeoEntityId = GeoEntityId(9);
pub const DE_W_A: GeoEntityId = GeoEntityId(10);

/// An id no entity in the fixture uses.
pub const UNKNOWN: GeoEntityId = GeoEntityId(999);

/// Every id set a caller can realistically ask for, named after the
/// `region.rs` query that produces it.
pub fn probes() -> Vec<(&'static str, Vec<GeoEntityId>)> {
    vec![
        ("continent level", vec![EU]),
        ("country level", vec![EU, FR, DE]),
        ("provinces of FR", vec![FR_N, FR_S]),
        ("provinces, all countries", vec![FR_N, FR_S, DE_W]),
        ("detail of FR", vec![FR, FR_N, FR_S]),
        ("detail of FR_N", vec![FR_N, FR_N_A, FR_N_B]),
        (
            "cities, all provinces",
            vec![FR_N_A, FR_N_B, FR_S_A, DE_W_A],
        ),
        ("DE subtree", vec![DE, DE_W, DE_W_A]),
        (
            "everything",
            vec![EU, FR, DE, FR_N, FR_S, DE_W, FR_N_A, FR_N_B, FR_S_A, DE_W_A],
        ),
        ("unvisited only", vec![UNKNOWN]),
    ]
}

/// Cities own whole tiles; every level above is reached by ancestor roll-up
/// inside `attribute`.
pub fn synthetic_geo_bytes() -> Vec<u8> {
    let entity = |id: u32, kind, code: &str, parent: Option<GeoEntityId>| GeoEntity {
        id: GeoEntityId(id),
        kind,
        canonical_code: code.into(),
        iso_a3_eh: None,
        name_key: format!("k.{code}"),
        parent_id: parent,
        total_area_m2: 1_000_000,
    };
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Admin0, "FRA", Some(EU)),
        entity(3, GeoEntityKind::Admin0, "DEU", Some(EU)),
        entity(4, GeoEntityKind::Admin1, "FR-N", Some(FR)),
        entity(5, GeoEntityKind::Admin1, "FR-S", Some(FR)),
        entity(6, GeoEntityKind::Admin1, "DE-W", Some(DE)),
        entity(7, GeoEntityKind::Admin2, "FR-N-A", Some(FR_N)),
        entity(8, GeoEntityKind::Admin2, "FR-N-B", Some(FR_N)),
        entity(9, GeoEntityKind::Admin2, "FR-S-A", Some(FR_S)),
        entity(10, GeoEntityKind::Admin2, "DE-W-A", Some(DE_W)),
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[geo_data_format::tile_index(0, 0)] = TileMembership::Single(FR_N_A);
    tiles[geo_data_format::tile_index(1, 0)] = TileMembership::Single(FR_N_B);
    tiles[geo_data_format::tile_index(2, 0)] = TileMembership::Single(FR_S_A);
    tiles[geo_data_format::tile_index(3, 0)] = TileMembership::Single(DE_W_A);
    write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap()
}

pub fn one_block(tile: TileKey, block: BlockKey, bits: u32) -> JourneyBitmap {
    let mut bm = JourneyBitmap::new();
    let mut b = Block::new();
    for i in 0..bits {
        b.set_point((i % 64) as u8, (i / 64) as u8, true);
    }
    bm.get_tile_mut_or_insert_empty(&tile).set(&block, b);
    bm
}

pub fn sub(dir: &TempDir, s: &str) -> String {
    let p = dir.path().join(s);
    fs::create_dir_all(&p).unwrap();
    p.into_os_string().into_string().unwrap()
}

/// Per layer, per probe: the visited areas a backend reports.
pub type Answers = Vec<(AchievementLayer, &'static str, Vec<(GeoEntityId, u64)>)>;

pub fn read_on_demand(storage: &Storage, geo: &dyn GeoLookup) -> Answers {
    storage
        .with_journey_snapshot(|snap| {
            let mut out = Answers::new();
            for layer in AchievementLayer::ALL_LAYERS {
                for (name, ids) in probes() {
                    let mut areas: Vec<_> = region_areas_from_snapshot(snap, geo, layer, &ids)?
                        .into_iter()
                        .collect();
                    areas.sort();
                    out.push((layer, name, areas));
                }
            }
            Ok(out)
        })
        .unwrap()
}

/// (tile, kind, day, bits) for the four fixture journeys: two countries, two
/// journey kinds, no two journeys sharing a tile, so no layer is empty and no
/// country holds the whole coverage.
pub fn journey_plan() -> [(TileKey, JourneyKind, u32, u32); 4] {
    [
        (TileKey::new(0, 0), JourneyKind::DefaultKind, 1, 30),
        (TileKey::new(2, 0), JourneyKind::DefaultKind, 2, 17),
        (TileKey::new(3, 0), JourneyKind::Flight, 3, 41),
        (TileKey::new(1, 0), JourneyKind::DefaultKind, 4, 9),
    ]
}

pub fn insert_one(
    storage: &Storage,
    tile: TileKey,
    kind: JourneyKind,
    day: u32,
    bits: u32,
) -> String {
    let bm = one_block(tile, BlockKey::from_x_y(1, 1), bits);
    storage
        .with_db_txn(|txn| {
            txn.create_and_insert_journey(
                NaiveDate::from_ymd_opt(2025, 1, day).unwrap(),
                None,
                None,
                None,
                kind,
                None,
                JourneyData::Bitmap(bm),
            )
        })
        .unwrap()
}

pub fn insert_journeys(storage: &Storage) -> Vec<String> {
    journey_plan()
        .into_iter()
        .map(|(tile, kind, day, bits)| insert_one(storage, tile, kind, day, bits))
        .collect()
}

pub fn new_storage(temp_dir: &TempDir, geo_bytes: &[u8]) -> Storage {
    let storage = Storage::init(
        sub(temp_dir, "temp/"),
        sub(temp_dir, "doc/"),
        sub(temp_dir, "support/"),
        sub(temp_dir, "cache/"),
    )
    .unwrap();
    storage
        .init_or_change_geo_data(Worldview::Iso, geo_bytes)
        .unwrap();
    storage
}
