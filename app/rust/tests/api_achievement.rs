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

use flutter_rust_bridge::DartFnFuture;
use geo_data_format::{
    read_header, write_geo_data_planes_with_display, DisplayBounds, DisplayPolygons, DisplayRecord,
    Worldview as GeoWorldview,
};
use memolanes_core::{
    api::achievement::{
        activate_geo_data, get_explored_area, get_explored_area_by_layer, region_detail,
        region_level_view, region_levels, AchievementLayer, GeoEntityId, RegionKind, Worldview,
    },
    api::api,
    api::import::JourneyInfo,
    gps::RawGPSPoint,
    import_data,
    journey_header::JourneyKind,
};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempdir::TempDir;

/// Import one GPX track as a finalized journey via the public GPS ingest path.
fn import_gpx_as_journey(path: &str) {
    let (raw, _pre) = import_data::gpx::load_gpx(path).unwrap();
    let points: Vec<RawGPSPoint> = raw
        .into_iter()
        .flatten()
        .filter(|p| p.timestamp_ms.is_some())
        .collect();
    assert!(!points.is_empty(), "no timestamped points in {path}");
    for p in &points {
        test_utils::record_gpx_point(p);
    }
    assert!(api::finalize_ongoing_journey().unwrap(), "finalize {path}");
}

#[test]
fn display_follows_the_active_worldview() {
    let dir = TempDir::new("display_worldviews").unwrap();
    let storage = geo_install_fixture::storage(&dir);
    let indonesia = GeoEntityId(1);
    assert!(storage.region_geometry(indonesia).is_err());
    for worldview in [GeoWorldview::Iso, GeoWorldview::Usa, GeoWorldview::Iso] {
        let path = test_utils::geo_asset(worldview.spec().id);
        let provenance = read_header(&path).unwrap().unwrap().provenance_hash;
        if !storage
            .open_installed_geo_data(worldview, provenance)
            .unwrap()
        {
            storage
                .init_or_change_geo_data(worldview, provenance, &fs::read(path).unwrap())
                .unwrap();
        }
        let shape = storage.region_geometry(indonesia).unwrap().unwrap();
        assert_eq!(shape.entity_id, indonesia);
        assert_eq!(shape.worldview, Worldview::from(worldview));
        let feature: serde_json::Value = serde_json::from_str(&shape.feature_geojson).unwrap();
        assert_eq!(feature["geometry"]["type"], "MultiPolygon");
        let polygons = feature["geometry"]["coordinates"].as_array().unwrap();
        assert!(polygons.len() > 1);
        let position = polygons[0][0][0].as_array().unwrap();
        assert_eq!(position.len(), 2);
        assert!(position.iter().all(serde_json::Value::is_number));
        assert!(storage
            .region_geometry(GeoEntityId(u32::MAX))
            .unwrap()
            .is_none());
    }
    let files = fs::read_dir(
        storage
            .installed_geo_data_file(GeoWorldview::Iso)
            .parent()
            .unwrap(),
    )
    .unwrap()
    .map(|entry| entry.unwrap().file_name())
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        files,
        ["geo_data_iso.bin".into(), "geo_data_usa.bin".into()].into()
    );
}

#[test]
fn corrupt_display_frame_does_not_disable_geo_achievements() {
    let dir = TempDir::new("display_frame_isolation").unwrap();
    let storage = geo_install_fixture::storage(&dir);
    let worldview = GeoWorldview::Iso;
    let raster = geo_data_format::GeoData::open(&test_utils::geo_asset("iso")).unwrap();
    let entities = raster.entities().unwrap();
    let id = GeoEntityId(1);
    let mut records = vec![DisplayRecord {
        worldview,
        entity_id: id,
        bounds: DisplayBounds {
            west: 0.0,
            south: 0.0,
            east: 1.0,
            north: 1.0,
        },
        polygons: DisplayPolygons::Exact(vec![vec![vec![
            [0.0, 0.0],
            [0.000_000_125, 0.0],
            [1.0, 1.0],
            [0.0, 0.0],
        ]]]),
    }];
    for entity in entities.iter().filter(|entity| {
        entity.id != id && entity.kind != geo_data_format::GeoEntityKind::Continent
    }) {
        let geometry = raster.display_geometry(entity.id).unwrap().unwrap();
        records.push(DisplayRecord {
            worldview,
            entity_id: entity.id,
            bounds: geometry.bounds,
            polygons: geometry.polygons,
        });
    }
    let hash = [42; 32];
    let bytes = write_geo_data_planes_with_display(&entities, "iso", &[], hash, &records).unwrap();
    storage
        .init_or_change_geo_data(worldview, hash, &bytes)
        .unwrap();
    let geometry = storage.region_geometry(id).unwrap().unwrap();
    let geojson: serde_json::Value = serde_json::from_str(&geometry.feature_geojson).unwrap();
    assert_eq!(
        geojson,
        serde_json::json!({
            "type": "Feature",
            "id": 1,
            "properties": { "entity_id": 1 },
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[0.0, 0.0], [0.000_000_125, 0.0], [1.0, 1.0], [0.0, 0.0]]]
            }
        })
    );
    let detail = || {
        storage
            .with_achievement_read(|store| {
                let ids = memolanes_core::achievement::region::detail_scope(store.geo()?, id);
                let areas = store.region_areas(AchievementLayer::Default, &ids)?;
                let detail =
                    memolanes_core::achievement::region::detail_view(store.geo()?, id, &areas)?
                        .expect("fixture must include Indonesia");
                Ok((
                    detail.node.total_area_m2,
                    detail.node.visited_area_m2,
                    detail.children.len(),
                ))
            })
            .unwrap()
    };
    let before = detail();
    {
        use std::io::{Seek, SeekFrom, Write};
        let path = storage.installed_geo_data_file(worldview);
        let display = read_header(&path).unwrap().unwrap().display.off;
        let frames = u64::from_le_bytes(bytes[display + 16..display + 24].try_into().unwrap());
        let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
        let first_frame = display as u64 + frames;
        file.seek(SeekFrom::Start(first_frame)).unwrap();
        file.write_all(&[bytes[first_frame as usize] ^ 1]).unwrap();
    }
    assert!(storage.region_geometry(id).is_err());
    assert_eq!(detail(), before);
    assert!(!storage.open_installed_geo_data(worldview, hash).unwrap());
    assert_eq!(detail(), before);
    storage
        .init_or_change_geo_data(worldview, hash, &bytes)
        .unwrap();
    assert!(storage.region_geometry(id).unwrap().is_some());
}

#[test]
fn malformed_display_span_is_retryable_without_losing_installed_coverage() {
    let dir = TempDir::new("display_header_repair").unwrap();
    let worldview = GeoWorldview::Iso;
    let path = test_utils::geo_asset("iso");
    let hash = read_header(&path).unwrap().unwrap().provenance_hash;
    let bytes = fs::read(path).unwrap();
    let mut damaged = bytes.clone();
    damaged[geo_data_format::HEADER_LEN - 8] ^= 1;
    let id = GeoEntityId(1);

    let storage = geo_install_fixture::storage(&dir);
    storage
        .init_or_change_geo_data(worldview, hash, &bytes)
        .unwrap();
    let installed = storage.installed_geo_data_file(worldview);
    assert!(storage
        .init_or_change_geo_data(worldview, hash, &damaged)
        .is_err());
    assert_eq!(fs::read(&installed).unwrap(), bytes);
    drop(storage);

    fs::write(&installed, &damaged).unwrap();
    let storage = geo_install_fixture::storage(&dir);
    assert!(storage.open_installed_geo_data(worldview, hash).unwrap());
    assert!(storage.region_geometry(id).is_err());
    assert!(!storage.open_installed_geo_data(worldview, hash).unwrap());
    assert!(storage
        .init_or_change_geo_data(worldview, hash, &damaged)
        .is_err());
    storage
        .with_achievement_read(|reader| {
            assert!(reader.geo()?.node(id).is_some());
            Ok(())
        })
        .unwrap();
    assert_eq!(fs::read(&installed).unwrap(), damaged);

    storage
        .init_or_change_geo_data(worldview, hash, &bytes)
        .unwrap();
    assert!(storage.region_geometry(id).unwrap().is_some());
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
        move || -> DartFnFuture<anyhow::Result<Vec<u8>>> {
            loads.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::ready(fs::read(&asset).map_err(Into::into)))
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

    // A failed bundle read can still activate a known-damaged installation
    // when it already supplies the requested worldview's coverage.
    let installed = dir.path().join("support/geo/geo_data_iso.bin");
    let mut bytes = fs::read(&installed).unwrap();
    let display = read_header(&installed).unwrap().unwrap().display.off;
    let frames = u64::from_le_bytes(bytes[display + 16..display + 24].try_into().unwrap());
    bytes[display + frames as usize] ^= 1;
    fs::write(&installed, bytes).unwrap();
    let id = GeoEntityId(1);
    assert!(memolanes_core::api::achievement::region_geometry(id).is_err());
    let missing_asset = || -> DartFnFuture<anyhow::Result<Vec<u8>>> {
        Box::pin(std::future::ready(Err(anyhow::anyhow!("missing bundle"))))
    };
    assert!(activate_geo_data(Worldview::Usa, missing_asset)
        .await
        .is_err());
    activate_geo_data(Worldview::Iso, missing_asset)
        .await
        .unwrap();
    assert!(region_detail(id, Default).unwrap().is_some());
    activate_geo_data(Worldview::Iso, &load_asset)
        .await
        .unwrap();
    assert!(memolanes_core::api::achievement::region_geometry(id)
        .unwrap()
        .is_some());
}
