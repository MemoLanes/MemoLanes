//! The compute-on-demand `OnDemandStore` reads explored area and region states
//! from a journey snapshot, with distinct per-layer areas, `All` as the true
//! union, and country→continent rollup.

use std::collections::BTreeMap;
use std::fs;

use chrono::NaiveDate;
use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview, TILE_COUNT,
};
use memolanes_core::{
    achievement::{
        backend::on_demand::OnDemandStore, contract::AchievementStore, layer::AchievementLayer,
    },
    geo::GeoIndex,
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
        iso_code: iso.into(),
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: 1_000_000,
    };
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Country, "FR", Some(1)),
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

/// Read area for all layers + region states through the on-demand store,
/// over one consistent snapshot from `storage`.
fn read_on_demand(
    storage: &Storage,
    oss: &OnDemandStore,
) -> (
    [u64; 3],
    memolanes_core::achievement::compute::region_state::RegionStateMap,
) {
    storage
        .with_journey_snapshot(|snap| {
            let reader = oss.reader(snap)?;
            let areas = [
                reader.explored_area_m2(AchievementLayer::Default)?,
                reader.explored_area_m2(AchievementLayer::Flight)?,
                reader.explored_area_m2(AchievementLayer::All)?,
            ];
            Ok((areas, reader.region_states(&AchievementLayer::ALL_LAYERS)?))
        })
        .unwrap()
}

#[test]
fn on_demand_areas_and_region_states() {
    let temp_dir = TempDir::new("test_on_demand").unwrap();
    let geo_bytes = synthetic_geo_bytes();
    let storage = Storage::init(
        sub(&temp_dir, "temp/"),
        sub(&temp_dir, "doc/"),
        sub(&temp_dir, "support/"),
        sub(&temp_dir, "cache/"),
    );
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

    // On-demand store with the same worldview geo.
    let mut oss = OnDemandStore::new();
    oss.set_geo(
        Worldview::Iso,
        Box::new(GeoIndex::from_bytes(&geo_bytes).unwrap()),
    )
    .unwrap();

    let (oss_areas, oss_states) = read_on_demand(&storage, &oss);

    // The data is non-trivial: distinct per-layer areas, All is the union,
    // France visited in every layer, rolled up to EU.
    let [default_area, flight_area, all_area] = oss_areas;
    assert!(default_area > 0 && flight_area > 0);
    assert_eq!(all_area, default_area + flight_area);
    assert_eq!(
        oss_states[&(AchievementLayer::Default, FR)].visited_area_m2,
        default_area
    );
    assert_eq!(
        oss_states[&(AchievementLayer::Flight, FR)].visited_area_m2,
        flight_area
    );
    assert_eq!(
        oss_states[&(AchievementLayer::All, EU)].visited_area_m2,
        all_area
    );
}

#[test]
fn on_demand_without_geo_has_no_regions() {
    let temp_dir = TempDir::new("test_on_demand_no_geo").unwrap();
    let storage = Storage::init(
        sub(&temp_dir, "temp/"),
        sub(&temp_dir, "doc/"),
        sub(&temp_dir, "support/"),
        sub(&temp_dir, "cache/"),
    );
    insert(
        &storage,
        (2025, 1, 1),
        JourneyKind::DefaultKind,
        one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 25),
    );

    // No geo supplied: region states empty, but explored area still computes.
    let oss = OnDemandStore::new();
    let (areas, states) = read_on_demand(&storage, &oss);
    assert!(states.is_empty());
    assert!(areas[0] > 0);
}
