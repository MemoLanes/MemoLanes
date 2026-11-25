#![cfg(geo_bins_present)]
//! End-to-end over the Flutter Rust Bridge–facing achievement API (`src/api/achievement.rs`),
//! driven through the global app state exactly as the Flutter layer would.
//!
//! The backend is whichever one the live `cache_db` provides, exercised through
//! the public api surface.
//!
//! Journeys are seeded through the real public ingest path (GPS points →
//! finalize), then read back via `get_explored_area*`. The geo initializer and
//! the region entry points are exercised against the real
//! `assets/geo/geo_data_iso.bin` worldview asset (skipped when it is absent).

mod geo_install_fixture;
pub mod test_utils;

use std::collections::HashMap;
use std::future::{poll_fn, ready, Future};
use std::pin::pin;
use std::task::Poll;

use flutter_rust_bridge::DartFnFuture;
use geo_data_format::{
    write_display_asset, DisplayBounds, DisplayRecord, Worldview as GeoWorldview,
};
use memolanes_core::storage::Storage;
use memolanes_core::{
    api::achievement::{
        activate_geo_data, get_explored_area, get_explored_area_by_layer, region_detail,
        region_geometry, region_level_view, region_levels, AchievementLayer, GeoEntityId,
        RegionKind, Worldview,
    },
    api::api,
    api::import::JourneyInfo,
    gps::{ExtendedRawGPSPoint, RawGPSPoint},
    import_data,
    journey_header::JourneyKind,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempdir::TempDir;
use tokio::sync::oneshot;

/// Import one GPX track as a finalized journey via the public GPS ingest path.
fn import_gpx_as_journey(path: &str) {
    let (raw, _pre) = import_data::gpx::load_gpx(path).unwrap();
    let points: Vec<RawGPSPoint> = raw
        .into_iter()
        .flatten()
        .filter(|p| p.timestamp_ms.is_some())
        .collect();
    assert!(!points.is_empty(), "no timestamped points in {path}");
    for p in points {
        let received_timestamp_ms = p.timestamp_ms.unwrap();
        api::on_location_update(ExtendedRawGPSPoint {
            raw_gps_point: p,
            received_timestamp_ms,
        });
    }
    assert!(api::finalize_ongoing_journey().unwrap(), "finalize {path}");
}

// A wrong worldview, a reused block from the prior raster, or using display
// errors to discard geo would break these user-visible assertions.
#[tokio::test]
async fn region_geometry_rejects_worldview_switch_then_reuses_installed_display() {
    let dir = TempDir::new("display_worldviews").unwrap();
    let storage = geo_install_fixture::storage(&dir);
    let id = GeoEntityId(1);
    let raster = |worldview: GeoWorldview| {
        let bytes = fs::read(test_utils::geo_asset(worldview.spec().id)).unwrap();
        let hash =
            geo_data_format::read_provenance_hash(&test_utils::geo_asset(worldview.spec().id))
                .unwrap()
                .unwrap();
        (bytes, hash)
    };
    let (iso, iso_hash) = raster(GeoWorldview::Iso);
    let (usa, usa_hash) = raster(GeoWorldview::Usa);
    let geometry = |worldview, west: f64| DisplayRecord {
        worldview,
        entity_id: id,
        bounds: DisplayBounds {
            west,
            south: 1.0,
            east: west + 1.0,
            north: 2.0,
        },
        polygons: vec![vec![vec![
            [west as i32 * 100_000, 100_000],
            [(west as i32 + 1) * 100_000, 100_000],
            [(west as i32 + 1) * 100_000, 200_000],
            [west as i32 * 100_000, 100_000],
        ]]],
    };
    let asset = write_display_asset(
        &[(GeoWorldview::Iso, iso_hash), (GeoWorldview::Usa, usa_hash)],
        &[
            geometry(GeoWorldview::Iso, 3.0),
            geometry(GeoWorldview::Usa, 7.0),
        ],
    )
    .unwrap();
    let digest: [u8; 32] = Sha256::digest(&asset).into();
    storage
        .init_or_change_geo_data(GeoWorldview::Iso, iso_hash, &iso)
        .unwrap();
    let unexpected_load = || -> std::future::Ready<anyhow::Result<Vec<u8>>> {
        panic!("should not load the display bundle")
    };
    {
        let (release, resume) = oneshot::channel::<()>();
        let mut loading = pin!(storage.region_geometry(id, digest, || async {
            resume.await.unwrap();
            Ok(asset.clone())
        }));
        let mut queued = pin!(storage.region_geometry(id, digest, unexpected_load));
        poll_fn(|cx| {
            assert!(loading.as_mut().poll(cx).is_pending());
            assert!(queued.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        storage
            .init_or_change_geo_data(GeoWorldview::Usa, usa_hash, &usa)
            .unwrap();
        release.send(()).unwrap();
        let (loading, queued) = tokio::join!(loading, queued);
        for error in [loading.unwrap_err(), queued.unwrap_err()] {
            assert_eq!(
                error.to_string(),
                "active raster changed during display lookup"
            );
        }
        assert!(!display_file(&storage, digest).exists());
    }
    let check = |found: memolanes_core::api::achievement::RegionGeometry, worldview, west| {
        assert_eq!(found.worldview, worldview);
        assert_eq!(found.bounds.west, west);
        assert_eq!(found.entity_id, id);
        let feature: serde_json::Value = serde_json::from_str(&found.feature_geojson).unwrap();
        assert_eq!(feature["id"], id.0);
        assert_eq!(feature["geometry"]["coordinates"][0][0][0], west);
        assert_eq!(found.display_dataset_version, hex::encode(digest));
    };
    let found = storage
        .region_geometry(id, digest, || ready(Ok(asset)))
        .await
        .unwrap()
        .unwrap();
    check(found, Worldview::Usa, 7.0);
    assert!(display_file(&storage, digest).exists());
    assert!(storage
        .region_geometry(GeoEntityId(u32::MAX), digest, unexpected_load)
        .await
        .unwrap()
        .is_none());
    drop(storage);

    let reopened = geo_install_fixture::storage(&dir);
    assert!(reopened
        .open_installed_geo_data(GeoWorldview::Iso, iso_hash)
        .unwrap());
    let found = reopened
        .region_geometry(id, digest, unexpected_load)
        .await
        .unwrap()
        .unwrap();
    check(found, Worldview::Iso, 3.0);
}

fn display_file(storage: &Storage, digest: [u8; 32]) -> std::path::PathBuf {
    storage
        .installed_geo_data_file(GeoWorldview::Iso)
        .with_file_name(format!("region_display_{}.bin", hex::encode(digest)))
}

#[tokio::test]
async fn region_geometry_rejects_provenance_switch_within_worldview() {
    use geo_install_fixture::{asset, storage, HASH_A, HASH_B};
    let dir = TempDir::new("display_provenance").unwrap();
    let storage = storage(&dir);
    let worldview = GeoWorldview::Iso;
    storage
        .init_or_change_geo_data(worldview, HASH_A, &asset(HASH_A))
        .unwrap();
    let bytes = write_display_asset(&[(worldview, HASH_B)], &[]).unwrap();
    let digest = Sha256::digest(&bytes).into();
    let id = GeoEntityId(1);
    let (release, resume) = oneshot::channel::<()>();
    let mut loading = pin!(storage.region_geometry(id, digest, || async {
        resume.await.unwrap();
        Ok(bytes.clone())
    }));
    poll_fn(|cx| {
        assert!(loading.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    storage
        .init_or_change_geo_data(worldview, HASH_B, &asset(HASH_B))
        .unwrap();
    release.send(()).unwrap();
    assert_eq!(
        loading.await.unwrap_err().to_string(),
        "active raster changed during display lookup"
    );
    assert!(!display_file(&storage, digest).exists());
    assert!(storage
        .region_geometry(id, digest, || ready(Ok(bytes.clone())))
        .await
        .unwrap()
        .is_none());
    assert!(display_file(&storage, digest).exists());
}

#[tokio::test]
async fn region_geometry_retries_after_loader_and_install_errors() {
    use geo_install_fixture::{asset, storage, HASH_A};
    let dir = TempDir::new("display_retry").unwrap();
    let storage = storage(&dir);
    let worldview = GeoWorldview::Iso;
    storage
        .init_or_change_geo_data(worldview, HASH_A, &asset(HASH_A))
        .unwrap();
    let bytes = write_display_asset(&[(worldview, HASH_A)], &[]).unwrap();
    let digest = Sha256::digest(&bytes).into();
    let id = GeoEntityId(1);
    let error = storage
        .region_geometry(id, digest, || async { anyhow::bail!("bundle unavailable") })
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "bundle unavailable");
    assert!(!display_file(&storage, digest).exists());
    let error = storage
        .region_geometry(id, digest, || ready(Ok(vec![0; bytes.len()])))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "display bundle digest mismatch");
    assert!(!display_file(&storage, digest).exists());
    assert!(storage
        .region_geometry(id, digest, || ready(Ok(bytes)))
        .await
        .unwrap()
        .is_none());
    assert!(display_file(&storage, digest).exists());
}

#[tokio::test]
async fn region_geometry_isolates_and_repairs_a_bad_display_frame() {
    let dir = TempDir::new("display_frame_isolation").unwrap();
    let sub = |s: &str| {
        let path = dir.path().join(s);
        fs::create_dir(&path).unwrap();
        path.to_string_lossy().into_owned()
    };
    let storage = Storage::init(sub("temp"), sub("doc"), sub("support"), sub("cache")).unwrap();
    let worldview = GeoWorldview::Iso;
    let raster = test_utils::geo_asset(worldview.spec().id);
    let hash = geo_data_format::read_provenance_hash(&raster)
        .unwrap()
        .unwrap();
    storage
        .init_or_change_geo_data(worldview, hash, &fs::read(raster).unwrap())
        .unwrap();
    let records = [1, 2].map(|id| DisplayRecord {
        worldview,
        entity_id: GeoEntityId(id),
        bounds: DisplayBounds {
            west: 0.0,
            south: 0.0,
            east: 1.0,
            north: 1.0,
        },
        polygons: vec![vec![vec![
            [0, 0],
            [100_000, 0],
            [100_000, 50_000 * id as i32],
            [0, 0],
        ]]],
    });
    let bytes = write_display_asset(&[(worldview, hash)], &records).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    assert!(storage
        .region_geometry(GeoEntityId(1), digest, || ready(Ok(bytes.clone())))
        .await
        .unwrap()
        .is_some());

    // Damage A's first compressed byte after the asset has been opened.
    use std::io::{Seek, SeekFrom, Write};
    let frame_offset = u64::from_le_bytes(bytes[56..64].try_into().unwrap());
    let path = display_file(&storage, digest);
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(frame_offset)).unwrap();
    file.write_all(&[0xff]).unwrap();
    drop(file);
    let unexpected_load = || -> std::future::Ready<anyhow::Result<Vec<u8>>> {
        panic!("must read the original frame without repair")
    };
    // Read B while A is still corrupt and before any failure requests repair.
    assert_eq!(
        storage
            .region_geometry(GeoEntityId(2), digest, unexpected_load)
            .await
            .unwrap()
            .unwrap()
            .entity_id,
        GeoEntityId(2)
    );
    let error = storage
        .region_geometry(GeoEntityId(1), digest, unexpected_load)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "display: decompressing entity frame");
    assert!(storage
        .region_geometry(GeoEntityId(1), digest, || ready(Ok(bytes.clone())))
        .await
        .unwrap()
        .is_some());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert!(storage
        .region_geometry(GeoEntityId(1), digest, unexpected_load)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn stale_display_asset_does_not_affect_stats() {
    let dir = TempDir::new("stale_display").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir(&p).unwrap();
        p.to_string_lossy().into_owned()
    };
    let storage = Storage::init(sub("temp"), sub("doc"), sub("support"), sub("cache")).unwrap();
    let raster = test_utils::geo_asset("iso");
    let hash = geo_data_format::read_provenance_hash(&raster)
        .unwrap()
        .unwrap();
    storage
        .init_or_change_geo_data(GeoWorldview::Iso, hash, &fs::read(&raster).unwrap())
        .unwrap();
    let detail = || {
        storage
            .with_achievement_read(|store| {
                let ids =
                    memolanes_core::achievement::region::detail_scope(store.geo()?, GeoEntityId(1));
                let areas = store.region_areas(AchievementLayer::Default, &ids)?;
                memolanes_core::achievement::region::detail_view(
                    store.geo()?,
                    GeoEntityId(1),
                    &areas,
                )
            })
            .unwrap()
    };
    let before = detail().map(|value| {
        (
            value.node.total_area_m2,
            value.node.visited_area_m2,
            value.children.len(),
        )
    });
    assert!(before.is_some(), "fixture must exercise existing stats");
    let bytes = write_display_asset(&[(GeoWorldview::Iso, [42; 32])], &[]).unwrap();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let error = storage
        .region_geometry(GeoEntityId(1), digest, || ready(Ok(bytes)))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "display raster provenance mismatch");
    assert!(!display_file(&storage, digest).exists());
    assert_eq!(
        detail().map(|value| (
            value.node.total_area_m2,
            value.node.visited_area_m2,
            value.children.len()
        )),
        before
    );
}

fn areas() -> HashMap<AchievementLayer, u64> {
    get_explored_area_by_layer().unwrap()
}

#[tokio::test]
async fn api_achievement_explored_area_and_region_contract() {
    let dir = TempDir::new("api_achievement").unwrap();
    let sub = |s: &str| {
        let p = dir.path().join(s);
        fs::create_dir(&p).unwrap();
        p.into_os_string().into_string().unwrap()
    };
    api::init(sub("temp"), sub("doc"), sub("support"), sub("cache")).unwrap();

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
    // installed, so the region APIs error (rather than returning empties that
    // read as "you've visited nowhere"). ---
    // (`api::init` is a process-global singleton, so this binary keeps a single
    // test; the geo install below continues in the same state.)
    assert!(region_levels().is_err(), "no geo → error");
    assert!(
        region_level_view(Default, RegionKind::Admin0).is_err(),
        "no geo → error"
    );
    assert!(
        region_detail(GeoEntityId(1), Default).is_err(),
        "no geo → error"
    );

    // --- Geo initializer against the real ISO asset. ---
    let asset = test_utils::geo_asset("iso");
    let loads = Arc::new(AtomicUsize::new(0));
    let load_asset = {
        let loads = loads.clone();
        let asset = asset.clone();
        move || -> DartFnFuture<Vec<u8>> {
            loads.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::ready(fs::read(&asset).unwrap()))
        }
    };
    activate_geo_data(Worldview::Iso, &load_asset)
        .await
        .unwrap();
    assert_eq!(
        loads.load(Ordering::SeqCst),
        1,
        "no installed copy → asset loaded"
    );
    activate_geo_data(Worldview::Iso, &load_asset)
        .await
        .unwrap();
    assert_eq!(
        loads.load(Ordering::SeqCst),
        1,
        "installed copy reused → not loaded again"
    );
    // Geo is now installed, so the seeded journeys light up region reads.
    assert!(!region_levels().unwrap().is_empty(), "geo → region levels");

    // --- Unknown-entity contract: with geo installed, an id the asset doesn't
    // know (not "geo absent") must answer empty rather than error. ---
    let unknown = GeoEntityId(u32::MAX);
    assert!(
        region_detail(unknown, Default).unwrap().is_none(),
        "unknown entity → no detail, not an error"
    );
    assert!(
        region_level_view(Default, RegionKind::Admin1).is_err(),
        "below the country, regions are listed only under their parent"
    );

    // Indonesia (frozen country ID 1) has real geometry in the bundled ISO display.
    let indonesia = GeoEntityId(1);
    let display_loads = AtomicUsize::new(0);
    let (release, resume) = oneshot::channel::<()>();
    let resume = Mutex::new(Some(resume));
    let load_display = || -> DartFnFuture<Vec<u8>> {
        display_loads.fetch_add(1, Ordering::SeqCst);
        let resume = resume.lock().unwrap().take().expect("only one bundle load");
        Box::pin(async move {
            resume.await.unwrap();
            fs::read("../assets/geo/region_display.bin").unwrap()
        })
    };
    let mut first = pin!(region_geometry(indonesia, &load_display));
    let mut concurrent = pin!(region_geometry(indonesia, &load_display));
    poll_fn(|cx| {
        assert!(first.as_mut().poll(cx).is_pending());
        assert_eq!(display_loads.load(Ordering::SeqCst), 1, "loader started");
        assert!(concurrent.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    release.send(()).unwrap();
    let (first, concurrent) = tokio::join!(first, concurrent);
    let warm = region_geometry(indonesia, &load_display).await;
    for result in [first, concurrent, warm] {
        let geometry = result
            .expect("lookup should prepare the display without activation")
            .expect("Indonesia has display geometry");
        assert_eq!(geometry.entity_id, indonesia);
        assert_eq!(geometry.worldview, Worldview::Iso);
    }
    assert!(region_geometry(unknown, &load_display)
        .await
        .unwrap()
        .is_none());
    assert_eq!(display_loads.load(Ordering::SeqCst), 1);
}
