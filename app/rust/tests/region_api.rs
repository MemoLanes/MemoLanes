use std::collections::BTreeMap;
use std::fs;

use chrono::NaiveDate;
use geo_data_format::{
    tile_index, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, TILE_COUNT,
};
use memolanes_core::{
    achievement::layer::AchievementLayer,
    achievement::read_model::region::{
        region_detail, region_level_view, region_levels, RegionKind,
    },
    journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    storage::Storage,
};
use tempdir::TempDir;

// EU(1) ⊃ { FR(2): all of tile (0,0); DE(3): one block of border tile (1,0) }.
fn geo_bytes() -> Vec<u8> {
    let ent = |id, kind, iso: &str, parent: Option<u32>| GeoEntity {
        id: GeoEntityId(id),
        kind,
        iso_code: iso.into(),
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: 1,
    };
    let entities = [
        ent(1, GeoEntityKind::Continent, "EU", None),
        ent(2, GeoEntityKind::Country, "FR", Some(1)),
        ent(3, GeoEntityKind::Country, "DE", Some(1)),
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[tile_index(0, 0)] = TileMembership::Single(GeoEntityId(2));
    tiles[tile_index(1, 0)] = TileMembership::Border;
    let mut cells = vec![None; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(7, 7).index()] = Some(GeoEntityId(3));
    let mut blocks = BTreeMap::new();
    blocks.insert((1, 0), cells);
    write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &blocks,
        [0u8; 32],
    )
    .unwrap()
}

fn one_block(tile: TileKey, block: BlockKey) -> JourneyBitmap {
    let mut bm = JourneyBitmap::new();
    let mut b = Block::new();
    for x in 0..20u8 {
        b.set_point(x, 0, true);
    }
    bm.get_tile_mut_or_insert_empty(&tile).set(&block, b);
    bm
}

fn insert(storage: &Storage, day: u32, kind: JourneyKind, bm: JourneyBitmap) {
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
        .unwrap();
}

#[test]
fn region_read_api_lists_progress_and_completion() {
    let dir = TempDir::new("region_api").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir_all(&p).unwrap();
        p.into_os_string().into_string().unwrap()
    };
    let storage = Storage::init(sub("t"), sub("d"), sub("s"), sub("c"));
    storage
        .init_or_change_geo_data(Worldview::Iso, &geo_bytes())
        .unwrap();

    // Default journey in France, Flight journey over Germany.
    insert(
        &storage,
        1,
        JourneyKind::DefaultKind,
        one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4)),
    );
    insert(
        &storage,
        2,
        JourneyKind::Flight,
        one_block(TileKey::new(1, 0), BlockKey::from_x_y(7, 7)),
    );

    storage
        .with_achievement_read(|store| {
            use AchievementLayer::*;
            let geo = store.geo().unwrap();
            let states = store.region_states(&AchievementLayer::ALL_LAYERS)?;

            // Levels: 1 continent, 2 countries.
            let levels = region_levels(geo);
            assert_eq!(levels[&RegionKind::Continent].region_count, 1);
            assert_eq!(levels[&RegionKind::Country].region_count, 2);

            let eu = Some(GeoEntityId(1));
            // Entries always list every country (FR, DE); only visited ones carry
            // area. Default sees only FR visited; All sees both.
            let def = region_level_view(&states, geo, Default, RegionKind::Country, eu);
            let mut def_ids: Vec<_> = def.entries.keys().copied().collect();
            def_ids.sort();
            assert_eq!(def_ids, vec![GeoEntityId(2), GeoEntityId(3)]);
            let mut def_visited: Vec<_> = def
                .entries
                .iter()
                .filter(|(_, e)| e.visited_area_m2 > 0)
                .map(|(&id, _)| id)
                .collect();
            def_visited.sort();
            assert_eq!(def_visited, vec![GeoEntityId(2)]);

            // Counts: a level is complete when visited_count == region_count > 0.
            assert_eq!((def.visited_count, def.region_count), (1, 2));
            let all = region_level_view(&states, geo, All, RegionKind::Country, eu);
            assert_eq!((all.visited_count, all.region_count), (2, 2));

            let world = region_level_view(&states, geo, All, RegionKind::Country, None);
            let mut world_ids: Vec<_> = world.entries.keys().copied().collect();
            world_ids.sort();
            assert_eq!(world_ids, vec![GeoEntityId(2), GeoEntityId(3)]);
            assert_eq!((world.visited_count, world.region_count), (2, 2));

            // Detail of EU (All layer): single-layer node + FR/DE children.
            let detail = region_detail(&states, geo, GeoEntityId(1), All).unwrap();
            assert_eq!(detail.entity_id, GeoEntityId(1));
            assert!(detail.node.visited_area_m2 > 0);
            let mut kids: Vec<_> = detail.children.keys().copied().collect();
            kids.sort();
            assert_eq!(kids, vec![GeoEntityId(2), GeoEntityId(3)]);

            Ok(())
        })
        .unwrap();
}

#[test]
fn init_or_change_geo_data_rejects_mismatched_worldview_id() {
    let dir = TempDir::new("test_geo_mismatch").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir_all(&p).unwrap();
        p.into_os_string().into_string().unwrap()
    };
    let storage = Storage::init(sub("t"), sub("d"), sub("s"), sub("c"));

    // A bin that declares "chn", loaded as Iso, must be rejected.
    let tiles = vec![TileMembership::None; TILE_COUNT];
    let bytes = write_geo_data(
        &[],
        Worldview::Chn.spec().id,
        &tiles,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap();
    let err = storage
        .init_or_change_geo_data(Worldview::Iso, &bytes)
        .expect_err("mismatched worldview id must be rejected");
    assert!(
        err.to_string().contains("declares worldview"),
        "unexpected error: {err}"
    );
}

#[test]
fn init_or_change_geo_data_rejects_invalid_bytes() {
    let dir = TempDir::new("test_geo_invalid_bytes").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir_all(&p).unwrap();
        p.into_os_string().into_string().unwrap()
    };
    let storage = Storage::init(sub("t"), sub("d"), sub("s"), sub("c"));

    assert!(storage
        .init_or_change_geo_data(Worldview::Iso, b"not a geo asset")
        .is_err());
}
