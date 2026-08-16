//! The compute-on-demand reads take explored area and region areas from a
//! journey snapshot, with distinct per-layer areas, `All` as the true union,
//! and country→continent rollup. The `AchievementReader` adapter over them
//! (including its no-worldview branch) is covered by `cache_db_v1.rs`.

use std::collections::{BTreeMap, HashMap};
use std::fs;

use chrono::NaiveDate;
use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview, TILE_COUNT,
};
use memolanes_core::{
    achievement::{
        layer::AchievementLayer,
        on_demand::{explored_areas_from_snapshot, region_areas_from_snapshot},
    },
    geo::{GeoIndex, GeoLookup},
    journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    storage::Storage,
};
use tempdir::TempDir;

const EU: GeoEntityId = GeoEntityId(1);
const FR: GeoEntityId = GeoEntityId(2);

/// Synthetic worldview asset: tile (0,0) is entirely France, a child of continent EU.
fn synthetic_geo_bytes() -> Vec<u8> {
    let entity = |id, kind, iso: &str, parent: Option<u32>| GeoEntity {
        id: GeoEntityId(id),
        kind,
        canonical_code: iso.into(),
        iso_a3_eh: None,
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: 1_000_000,
    };
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Admin0, "FR", Some(1)),
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[0] = TileMembership::Single(FR);
    write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap()
}

fn one_block(tile: TileKey, block: BlockKey, bits: u32) -> JourneyBitmap {
    let mut bm = JourneyBitmap::new();
    let mut b = Block::new();
    for i in 0..bits {
        b.set_point((i % 64) as u8, (i / 64) as u8, true);
    }
    bm.get_tile_mut_or_insert_empty(&tile).set(&block, b);
    bm
}

fn insert(storage: &Storage, date: (i32, u32, u32), kind: JourneyKind, bm: JourneyBitmap) {
    storage
        .with_db_txn(|txn| {
            txn.create_and_insert_journey(
                NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
                None,
                None,
                None,
                kind,
                None,
                JourneyData::Bitmap(bm),
            )
        })
        .unwrap();
}

fn sub(dir: &TempDir, s: &str) -> String {
    let p = dir.path().join(s);
    fs::create_dir_all(&p).unwrap();
    p.into_os_string().into_string().unwrap()
}

const LAYERS: [AchievementLayer; 3] = [
    AchievementLayer::Default,
    AchievementLayer::Flight,
    AchievementLayer::All,
];

/// Per-layer explored area, in `LAYERS` order.
fn explored_areas(storage: &Storage) -> [u64; 3] {
    storage
        .with_journey_snapshot(|snap| {
            let by_layer = explored_areas_from_snapshot(snap, &LAYERS)?;
            Ok(LAYERS.map(|layer| by_layer[&layer]))
        })
        .unwrap()
}

/// Read area for all layers + the `All`-layer areas of EU/FR over one
/// consistent snapshot from `storage`, so they cannot skew against each other.
fn read_on_demand(storage: &Storage, geo: &dyn GeoLookup) -> ([u64; 3], HashMap<GeoEntityId, u64>) {
    storage
        .with_journey_snapshot(|snap| {
            let by_layer = explored_areas_from_snapshot(snap, &LAYERS)?;
            Ok((
                LAYERS.map(|layer| by_layer[&layer]),
                region_areas_from_snapshot(snap, geo, AchievementLayer::All, &[EU, FR])?,
            ))
        })
        .unwrap()
}

#[test]
fn on_demand_areas_and_region_areas() {
    let temp_dir = TempDir::new("test_on_demand").unwrap();
    let geo_bytes = synthetic_geo_bytes();
    let storage = Storage::init(
        sub(&temp_dir, "temp/"),
        sub(&temp_dir, "doc/"),
        sub(&temp_dir, "support/"),
        sub(&temp_dir, "cache/"),
    )
    .unwrap();
    storage
        .init_or_change_geo_data(Worldview::Iso, &geo_bytes)
        .unwrap();

    // A Default journey and a Flight journey, in different blocks of France, so
    // `All` is a true union and each per-layer area is distinct.
    insert(
        &storage,
        (2025, 1, 1),
        JourneyKind::DefaultKind,
        one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 25),
    );
    insert(
        &storage,
        (2025, 1, 2),
        JourneyKind::Flight,
        one_block(TileKey::new(0, 0), BlockKey::from_x_y(5, 6), 40),
    );

    // On-demand reads against the same worldview geo.
    let geo = GeoIndex::from_bytes(&geo_bytes).unwrap();

    let (oss_areas, all_layer_regions) = read_on_demand(&storage, &geo);

    // The data is non-trivial: distinct per-layer areas, All is the union,
    // France visited in every layer, rolled up to EU.
    let [default_area, flight_area, all_area] = oss_areas;
    assert!(default_area > 0 && flight_area > 0);
    assert_eq!(all_area, default_area + flight_area);

    // France's Default and Flight areas are read per layer.
    let (fr_default, fr_flight) = storage
        .with_journey_snapshot(|snap| {
            Ok((
                region_areas_from_snapshot(snap, &geo, AchievementLayer::Default, &[FR])?,
                region_areas_from_snapshot(snap, &geo, AchievementLayer::Flight, &[FR])?,
            ))
        })
        .unwrap();
    assert_eq!(fr_default[&FR], default_area);
    assert_eq!(fr_flight[&FR], flight_area);
    // EU rolls up the union in the All layer.
    assert_eq!(all_layer_regions[&EU], all_area);
}

#[test]
fn on_demand_area_without_geo() {
    let temp_dir = TempDir::new("test_on_demand_no_geo").unwrap();
    let storage = Storage::init(
        sub(&temp_dir, "temp/"),
        sub(&temp_dir, "doc/"),
        sub(&temp_dir, "support/"),
        sub(&temp_dir, "cache/"),
    )
    .unwrap();
    insert(
        &storage,
        (2025, 1, 1),
        JourneyKind::DefaultKind,
        one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 25),
    );

    // Area needs no worldview installed; regions do (see `cache_db_v1.rs`).
    assert!(explored_areas(&storage)[0] > 0);
}
