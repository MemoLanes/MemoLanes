//! Pins `CacheDbV1`'s achievement wiring — `achievement_reader` — directly on
//! the concrete backend, independent of what `cache_db::new` returns. The
//! answers themselves belong to `OnDemandReader` and are covered by
//! `achievement_on_demand.rs`; what this suite pins is that `CacheDbV1` hands
//! it the right snapshot and the installed geo.

pub mod test_utils;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview, TILE_COUNT,
};
use memolanes_core::{
    achievement::layer::AchievementLayer,
    cache_db::{CacheDb, CacheDbV1},
    geo::GeoIndex,
    journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey},
    journey_header::JourneyKind,
    main_db::MainDb,
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

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A `MainDb` holding one Default and one Flight journey in different blocks of
/// France, beside a fresh `CacheDbV1`. Disjoint blocks make `All` a strict sum.
fn setup(prefix: &str) -> (MainDb, CacheDbV1, TempDir, TempDir) {
    let main_dir = TempDir::new(&format!("{prefix}-main")).unwrap();
    let cache_dir = TempDir::new(&format!("{prefix}-cache")).unwrap();
    let mut main_db = MainDb::open(main_dir.path().to_str().unwrap()).unwrap();
    let cache_db = CacheDbV1::open(cache_dir.path().to_str().unwrap());

    main_db
        .with_txn(|txn| {
            test_utils::insert_bitmap_journey(
                txn,
                date("2025-01-01"),
                JourneyKind::DefaultKind,
                one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 25),
            );
            test_utils::insert_bitmap_journey(
                txn,
                date("2025-01-02"),
                JourneyKind::Flight,
                one_block(TileKey::new(0, 0), BlockKey::from_x_y(5, 6), 40),
            );
            Ok(())
        })
        .unwrap();

    (main_db, cache_db, main_dir, cache_dir)
}

#[test]
fn achievement_reader_serves_areas_and_regions() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) = setup("cache_db_v1-reader");
    let geo = GeoIndex::from_bytes(&synthetic_geo_bytes()).unwrap();

    main_db
        .with_txn(|txn| {
            let mut reader = cache_db.achievement_reader(txn, Some(&geo))?;

            let default = reader.explored_area_m2(AchievementLayer::Default)?;
            let flight = reader.explored_area_m2(AchievementLayer::Flight)?;
            let all = reader.explored_area_m2(AchievementLayer::All)?;
            assert!(default > 0 && flight > 0);
            assert_eq!(all, default + flight);

            // The installed geo reached the reader: France is attributed per
            // layer and rolls up to EU in the union.
            let fr_default = reader.region_areas(AchievementLayer::Default, &[FR])?;
            assert_eq!(fr_default[&FR], default);
            let all_regions = reader.region_areas(AchievementLayer::All, &[EU, FR])?;
            assert_eq!(all_regions[&EU], all);
            assert_eq!(all_regions[&FR], all);

            assert!(reader.geo().is_ok());
            Ok(())
        })
        .unwrap();
}

#[test]
fn achievement_reader_without_geo_has_no_regions() {
    let (mut main_db, mut cache_db, _main_dir, _cache_dir) = setup("cache_db_v1-reader-no-geo");

    main_db
        .with_txn(|txn| {
            let mut reader = cache_db.achievement_reader(txn, None)?;
            // Area needs no worldview, regions do.
            assert!(reader.explored_area_m2(AchievementLayer::Default)? > 0);
            assert!(reader
                .region_areas(AchievementLayer::All, &[EU, FR])?
                .is_empty());
            assert!(reader.geo().is_err());
            Ok(())
        })
        .unwrap();
}
