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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &asset(HASH_A))
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
fn startup_removes_abandoned_geo_staging_files_without_touching_installed_data() {
    let dir = TempDir::new("geo_install").unwrap();
    let first = storage(&dir);
    let installed = first.installed_geo_data_file(Worldview::Iso);
    let iso_staging = installed.with_extension("bin.tmp");
    let usa_staging = first
        .installed_geo_data_file(Worldview::Usa)
        .with_extension("bin.tmp");
    let unrelated = installed.with_file_name("unrelated.tmp");
    let original = asset(HASH_A);
    first
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &original)
        .unwrap();
    drop(first);
    for path in [&iso_staging, &usa_staging, &unrelated] {
        fs::write(path, b"partial").unwrap();
    }

    let later = storage(&dir);
    assert!(!iso_staging.exists());
    assert!(!usa_staging.exists());
    assert!(unrelated.exists());
    assert!(later
        .open_installed_geo_data(Worldview::Iso, HASH_A)
        .unwrap());
    assert_eq!(fs::read(installed).unwrap(), original);
}

#[test]
fn concurrent_installs_leave_a_complete_file_and_matching_active_index() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    let rendezvous = std::sync::Barrier::new(2);
    let a = asset(HASH_A);
    let b = asset(HASH_B);
    std::thread::scope(|scope| {
        for (hash, bytes) in [(HASH_A, &a), (HASH_B, &b)] {
            let storage = &storage;
            let rendezvous = &rendezvous;
            scope.spawn(move || {
                rendezvous.wait();
                storage
                    .init_or_change_geo_data(Worldview::Iso, hash, bytes)
                    .unwrap();
            });
        }
    });
    let installed = storage.installed_geo_data_file(Worldview::Iso);
    let expected = match active_hash(&storage) {
        Some(HASH_A) => a,
        Some(HASH_B) => b,
        hash => panic!("unexpected active provenance: {hash:?}"),
    };
    assert_eq!(fs::read(&installed).unwrap(), expected);
    assert!(!installed.with_extension("bin.tmp").exists());
}

#[test]
fn installed_copy_with_a_different_provenance_is_not_opened() {
    let dir = TempDir::new("geo_install").unwrap();
    storage(&dir)
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &asset(HASH_A))
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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &asset(HASH_A))
        .unwrap();
    storage
        .init_or_change_geo_data(Worldview::Iso, HASH_B, &asset(HASH_B))
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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &asset(HASH_A))
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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, b"not a geo asset")
        .is_err());
    let installed = storage.installed_geo_data_file(Worldview::Iso);
    assert!(!installed.exists());
    assert!(!installed.with_extension("bin.tmp").exists());
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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &bytes)
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
        .init_or_change_geo_data(Worldview::Iso, HASH_A, &bytes)
        .unwrap();
    assert!(fr_area_m2(&third).unwrap() > 0);
}

#[test]
fn bytes_with_an_unexpected_provenance_are_rejected() {
    let dir = TempDir::new("geo_install").unwrap();
    let storage = storage(&dir);
    assert!(storage
        .init_or_change_geo_data(Worldview::Iso, HASH_B, &asset(HASH_A))
        .is_err());
    assert!(!storage.installed_geo_data_file(Worldview::Iso).exists());
    assert_eq!(active_hash(&storage), None);
}

fn asset_with_outline(worldview: Worldview) -> Vec<u8> {
    use geo_data_format::{
        write_geo_data_planes_with_display, DisplayBounds, DisplayPolygons, DisplayRecord,
        GeoEntity, GeoEntityKind,
    };
    let entity = GeoEntity {
        id: FR,
        kind: GeoEntityKind::Admin0,
        canonical_code: "FR".into(),
        iso_a3_eh: None,
        name_key: "k.FR".into(),
        parent_id: None,
        total_area_m2: 1,
    };
    let record = DisplayRecord {
        worldview,
        entity_id: FR,
        bounds: DisplayBounds {
            west: 0.0,
            south: 0.0,
            east: 1.0,
            north: 1.0,
        },
        polygons: DisplayPolygons::Exact(vec![vec![vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [0.0, 0.0],
        ]]]),
    };
    write_geo_data_planes_with_display(&[entity], worldview.spec().id, &[], HASH_A, &[record])
        .unwrap()
}

fn damage_outline(bytes: &[u8], installed: &std::path::Path) -> Vec<u8> {
    let display = geo_data_format::read_header(installed)
        .unwrap()
        .unwrap()
        .display
        .off;
    let frames = u64::from_le_bytes(bytes[display + 16..display + 24].try_into().unwrap());
    let mut damaged = bytes.to_vec();
    damaged[display + frames as usize] ^= 1;
    fs::write(installed, &damaged).unwrap();
    damaged
}

#[test]
fn known_display_damage_survives_worldview_changes_and_restart_until_repaired() {
    let dir = TempDir::new("geo_display_repair").unwrap();
    let iso = Worldview::Iso;
    let usa = Worldview::Usa;
    let bytes = asset_with_outline(iso);
    let first = storage(&dir);
    first.init_or_change_geo_data(iso, HASH_A, &bytes).unwrap();
    let installed = first.installed_geo_data_file(iso);
    let damaged = damage_outline(&bytes, &installed);
    assert!(first.region_geometry(FR).is_err());
    assert_eq!(active_hash(&first), Some(HASH_A));
    first
        .init_or_change_geo_data(usa, HASH_A, &asset_with_outline(usa))
        .unwrap();
    assert!(!first.open_installed_geo_data(iso, HASH_A).unwrap());
    assert_eq!(
        first.region_geometry(FR).unwrap().unwrap().worldview,
        usa.into()
    );
    drop(first);

    let restarted = storage(&dir);
    assert!(!restarted.open_installed_geo_data(iso, HASH_A).unwrap());
    assert_eq!(active_hash(&restarted), Some(HASH_A));
    assert!(restarted
        .init_or_change_geo_data(iso, HASH_A, &damaged)
        .is_err());
    assert_eq!(fs::read(&installed).unwrap(), damaged);
    assert!(!restarted.open_installed_geo_data(iso, HASH_A).unwrap());
    restarted
        .init_or_change_geo_data(iso, HASH_A, &bytes)
        .unwrap();
    drop(restarted);

    let repaired = storage(&dir);
    assert!(repaired.open_installed_geo_data(iso, HASH_A).unwrap());
    assert!(repaired.region_geometry(FR).unwrap().is_some());
}

#[test]
fn a_relabelled_outline_is_detected_and_cannot_clear_a_repair_request() {
    let dir = TempDir::new("geo_wrong_display_id").unwrap();
    let iso = Worldview::Iso;
    let bytes = asset_with_outline(iso);
    let first = storage(&dir);
    first.init_or_change_geo_data(iso, HASH_A, &bytes).unwrap();
    let path = first.installed_geo_data_file(iso);
    let display = geo_data_format::read_header(&path)
        .unwrap()
        .unwrap()
        .display
        .off;
    let index = u64::from_le_bytes(bytes[display..display + 8].try_into().unwrap()) as usize;
    let forged = GeoEntityId(2_000);
    let mut damaged = bytes.clone();
    let first_row = display + index;
    damaged[first_row..first_row + 4].copy_from_slice(&forged.0.to_le_bytes());
    fs::write(&path, &damaged).unwrap();
    assert!(first.region_geometry(FR).is_err());
    assert!(first.region_geometry(forged).is_err());
    drop(first);

    let restarted = storage(&dir);
    assert!(!restarted.open_installed_geo_data(iso, HASH_A).unwrap());
    assert!(restarted
        .init_or_change_geo_data(iso, HASH_A, &damaged)
        .is_err());
    assert_eq!(active_hash(&restarted), Some(HASH_A));
    restarted
        .init_or_change_geo_data(iso, HASH_A, &bytes)
        .unwrap();
    assert!(restarted.region_geometry(FR).unwrap().is_some());
    assert!(restarted.region_geometry(forged).unwrap().is_none());
}

#[test]
fn a_stale_display_read_cannot_mark_a_newly_installed_copy() {
    let dir = TempDir::new("geo_stale_display").unwrap();
    let iso = Worldview::Iso;
    let bytes = asset_with_outline(iso);
    let old = storage(&dir);
    old.init_or_change_geo_data(iso, HASH_A, &bytes).unwrap();
    damage_outline(&bytes, &old.installed_geo_data_file(iso));

    let replacement = storage(&dir);
    replacement
        .init_or_change_geo_data(iso, HASH_A, &bytes)
        .unwrap();
    assert!(old.region_geometry(FR).is_err());
    assert!(old.open_installed_geo_data(iso, HASH_A).unwrap());
    assert!(old.region_geometry(FR).unwrap().is_some());
    assert!(replacement.open_installed_geo_data(iso, HASH_A).unwrap());
}
