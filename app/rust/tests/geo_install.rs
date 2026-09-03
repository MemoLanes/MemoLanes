//! The geo asset's installed copy: `init_or_change_geo_data` persists the
//! bytes under the support dir and serves border tiles from that file, and
//! `open_installed_geo_data` reactivates the copy on a later launch without
//! the caller re-supplying the asset, as long as its provenance hash matches.

use std::collections::BTreeMap;
use std::fs;

use geo_data_format::{
    tile_index, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};
use memolanes_core::journey_bitmap::{BlockKey, TileKey};
use memolanes_core::storage::Storage;
use tempdir::TempDir;

const HASH_A: [u8; 32] = [0xA5; 32];
const HASH_B: [u8; 32] = [0x5A; 32];

fn asset(provenance_hash: [u8; 32]) -> Vec<u8> {
    let entities = [
        GeoEntity {
            id: GeoEntityId(1),
            kind: GeoEntityKind::Continent,
            canonical_code: "EU".into(),
            iso_a3_eh: None,
            name_key: "k.EU".into(),
            parent_id: None,
            total_area_m2: 1,
        },
        GeoEntity {
            id: GeoEntityId(2),
            kind: GeoEntityKind::Admin0,
            canonical_code: "FR".into(),
            iso_a3_eh: None,
            name_key: "k.FR".into(),
            parent_id: Some(GeoEntityId(1)),
            total_area_m2: 1,
        },
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[tile_index(0, 0)] = TileMembership::Single(GeoEntityId(2));
    tiles[tile_index(1, 0)] = TileMembership::Border;
    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(5, 5).index()] = 2;
    let mut blocks = BTreeMap::new();
    blocks.insert((1, 0), cells);
    write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &blocks,
        provenance_hash,
    )
    .unwrap()
}

fn sub(dir: &TempDir, name: &str) -> String {
    let p = dir.path().join(name);
    fs::create_dir_all(&p).unwrap();
    p.into_os_string().into_string().unwrap()
}

fn storage(dir: &TempDir) -> Storage {
    Storage::init(sub(dir, "t"), sub(dir, "d"), sub(dir, "s"), sub(dir, "c")).unwrap()
}

/// Provenance hash of the geo lookup the achievement reader currently sees.
fn active_hash(storage: &Storage) -> Option<[u8; 32]> {
    storage
        .with_achievement_read(|reader| Ok(reader.geo().ok().map(|geo| geo.provenance_hash())))
        .unwrap()
}

#[test]
fn nothing_is_installed_in_a_fresh_support_dir() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    assert!(!storage
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert_eq!(active_hash(&storage), None);
}

#[test]
fn installed_copy_is_reopened_by_a_later_storage_without_the_bytes() {
    let dir = TempDir::new("geo_install").unwrap();
    storage(&dir)
        .init_or_change_geo_data(Worldview::Iso, &asset(HASH_A))
        .unwrap();

    let later = storage(&dir);
    assert_eq!(active_hash(&later), None);
    assert!(later
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert_eq!(active_hash(&later), Some(HASH_A));

    // Border tiles are served from the installed file.
    let owner = later
        .with_achievement_read(|reader| {
            reader
                .geo()?
                .entity_of_block(TileKey::new(1, 0), BlockKey::from_x_y(5, 5))
        })
        .unwrap();
    assert_eq!(owner, Some(GeoEntityId(2)));
}

#[test]
fn installed_copy_with_a_different_hash_is_not_opened() {
    let dir = TempDir::new("geo_install").unwrap();
    storage(&dir)
        .init_or_change_geo_data(Worldview::Iso, &asset(HASH_A))
        .unwrap();

    let later = storage(&dir);
    assert!(!later
        .open_installed_geo_data(Worldview::Iso, HASH_B)
        .unwrap());
    assert_eq!(active_hash(&later), None);
}

#[test]
fn reinstalling_replaces_the_previous_copy() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    storage
        .init_or_change_geo_data(Worldview::Iso, &asset(HASH_A))
        .unwrap();
    storage
        .init_or_change_geo_data(Worldview::Iso, &asset(HASH_B))
        .unwrap();
    assert_eq!(active_hash(&storage), Some(HASH_B));

    let later = self::storage(&dir);
    assert!(!later
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert!(later
        .open_installed_geo_data(Worldview::Iso, HASH_B)
        .unwrap());
}

#[test]
fn a_corrupt_installed_copy_is_not_opened() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    storage
        .init_or_change_geo_data(Worldview::Iso, &asset(HASH_A))
        .unwrap();
    let path = storage.installed_geo_data_file(Worldview::Iso);
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();

    let later = self::storage(&dir);
    assert!(!later
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert_eq!(active_hash(&later), None);
}

#[test]
fn rejected_bytes_leave_no_installed_copy_behind() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    assert!(storage
        .init_or_change_geo_data(Worldview::Iso, b"not a geo asset")
        .is_err());
    assert!(!storage.installed_geo_data_file(Worldview::Iso).exists());
    assert!(!storage
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
}
