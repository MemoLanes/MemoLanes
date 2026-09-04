//! The geo asset's installed copy: `init_or_change_geo_data` persists the
//! bytes under the support dir and serves border tiles from that file, and
//! `open_installed_geo_data` reactivates the copy on a later launch, as long
//! as its provenance hash matches, without the caller re-supplying the asset.

mod geo_install_fixture;

use std::fs;

use geo_data_format::{GeoEntityId, Worldview};
use geo_install_fixture::*;
use tempdir::TempDir;

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

    let owner = later
        .with_achievement_read(|reader| reader.geo()?.entity_of_block(BORDER_TILE, fr_block()))
        .unwrap();
    assert_eq!(owner, Some(GeoEntityId(2)));
}

#[test]
fn installed_copy_with_a_different_provenance_is_not_opened() {
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
fn a_truncated_installed_copy_is_not_opened() {
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

/// Same-size corruption is not detected up front; the first read that hits it
/// fails, the copy is discarded, and the next launch reinstalls from the bundle.
#[test]
fn corrupt_border_blob_during_a_read_discards_the_installed_copy() {
    let dir = TempDir::new("geo_install").unwrap();
    let bytes = asset(HASH_A);
    let first = storage(&dir);
    first
        .init_or_change_geo_data(Worldview::Iso, &bytes)
        .unwrap();
    insert_border_journey(&first, 1, 10).unwrap();
    let path = first.installed_geo_data_file(Worldview::Iso);
    drop(first);
    corrupt_blob_region(&path);

    let second = storage(&dir);
    assert!(second
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert!(fr_area_m2(&second).is_err());
    assert_eq!(active_hash(&second), None);
    assert!(!path.exists());

    let third = storage(&dir);
    assert!(!third
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    third
        .init_or_change_geo_data(Worldview::Iso, &bytes)
        .unwrap();
    assert!(fr_area_m2(&third).unwrap() > 0);
}
