//! End-to-end ground-truth for the region achievement API over the *real*
//! Natural Earth ADM0 rasterized worldview asset (`assets/geo/geo_data_iso.bin`).
//!
//! The other region tests use hand-built synthetic geo, so they prove the read
//! logic is self-consistent but cannot prove it is *correct* against the world.
//! Here synthetic journeys are placed at real city coordinates whose country we
//! know independently (ADM0_A3), and we assert the API reports the CORRECT
//! countries and CORRECT areas.
//!
//! Areas are cross-checked against an INDEPENDENT closed-form spherical-zone
//! integral (R²·Δλ·(sinφ₁−sinφ₂)), distinct from the core's center-bit rectangle
//! method, so a wrong radius / missing latitude correction / wrong grid resolution
//! would be caught rather than mirrored.
//!
//! The asset is gitignored and produced by `just rasterize-geo`; the test skips
//! when it is absent (e.g. a clean CI without the data step).

pub mod test_utils;

use std::collections::{BTreeSet, HashMap};
use std::fs;

use chrono::NaiveDate;
use geo_data_format::{GeoEntityId, GeoEntityKind, Worldview};
use memolanes_core::{
    achievement::attribution::{self, AreaCm2ByEntity},
    achievement::layer::AchievementLayer,
    achievement::region::{self, region_levels, RegionKind},
    geo::{GeoIndex, GeoLookup},
    journey_bitmap::{
        Block, BlockKey, JourneyBitmap, TileKey, BITMAP_WIDTH_OFFSET, MAP_WIDTH_OFFSET,
        TILE_WIDTH_OFFSET,
    },
    journey_data::JourneyData,
    journey_header::JourneyKind,
    storage::Storage,
};
use tempdir::TempDir;

const EARTH_RADIUS: f64 = 6_371_000.0; // must match journey_area_utils
const BIT_OFFSET: i32 = (MAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET + BITMAP_WIDTH_OFFSET) as i32; // 22
const BLOCK_OFFSET: i32 = (MAP_WIDTH_OFFSET + TILE_WIDTH_OFFSET) as i32; // 16 -> 65536 blocks
const BITS_PER_TILE: i32 = 128 * 64; // blocks-per-tile * bits-per-block

// A patch of bits placed in each city's block, centered so the core's
// block-center area approximation samples ~the same latitude as our integral.
const PATCH_LO: u8 = 22;
const PATCH_HI: u8 = 42; // [22,42) -> 20x20 = 400 bits

struct City {
    name: &'static str,
    lng: f64,
    lat: f64,
    iso: &'static str, // ADM0_A3 ground truth
    kind: JourneyKind,
}

/// External ground truth: real city centers (well inside their borders) and the
/// ADM0_A3 country each sits in.
const CITIES: &[City] = &[
    City {
        name: "Paris",
        lng: 2.3522,
        lat: 48.8566,
        iso: "FRA",
        kind: JourneyKind::DefaultKind,
    },
    City {
        name: "Mexico City",
        lng: -99.1332,
        lat: 19.4326,
        iso: "MEX",
        kind: JourneyKind::DefaultKind,
    },
    City {
        name: "Sydney",
        lng: 151.2093,
        lat: -33.8688,
        iso: "AUS",
        kind: JourneyKind::DefaultKind,
    },
    City {
        name: "Nairobi",
        lng: 36.8219,
        lat: -1.2921,
        iso: "KEN",
        kind: JourneyKind::DefaultKind,
    },
    City {
        name: "Reykjavik",
        lng: -21.9426,
        lat: 64.1466,
        iso: "ISL",
        kind: JourneyKind::DefaultKind,
    },
    City {
        name: "Tokyo",
        lng: 139.6917,
        lat: 35.6895,
        iso: "JPN",
        kind: JourneyKind::Flight,
    },
];

/// Web-mercator (lng,lat) -> global block coords on the 65536-block grid (the
/// same projection the rasterizer used, so lookups are self-consistent).
fn block_of(lng: f64, lat: f64) -> (TileKey, BlockKey) {
    let n = f64::powi(2.0, BLOCK_OFFSET);
    let lat_rad: f64 = lat.to_radians();
    let x = (lng + 180.0) / 360.0 * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * n;
    let (bx, by) = (x as u32, y as u32);
    (
        TileKey::new((bx / 128) as u16, (by / 128) as u16),
        BlockKey::from_x_y((bx % 128) as u8, (by % 128) as u8),
    )
}

/// Global bit row -> latitude (deg) of that grid line.
fn bit_y_to_lat(gy: i32) -> f64 {
    let n = f64::powi(2.0, BIT_OFFSET);
    f64::atan(f64::sinh(
        std::f64::consts::PI * (1.0 - 2.0 * gy as f64 / n),
    ))
    .to_degrees()
}

/// Exact area (m²) of the centered PATCH in `(tile, block)`, by the closed-form
/// spherical-zone integral — independent of the core's area method.
fn expected_patch_area(tile: TileKey, block: BlockKey) -> f64 {
    let base_y = tile.y as i32 * BITS_PER_TILE + block.y() as i32 * 64;
    let ncols = (PATCH_HI - PATCH_LO) as f64;
    let dlon_rad = 2.0 * std::f64::consts::PI / f64::powi(2.0, BIT_OFFSET); // per bit column
    let mut area = 0.0;
    for ly in PATCH_LO..PATCH_HI {
        let gy = base_y + ly as i32;
        let lat_top = bit_y_to_lat(gy).to_radians();
        let lat_bottom = bit_y_to_lat(gy + 1).to_radians();
        area += EARTH_RADIUS * EARTH_RADIUS * dlon_rad * ncols * (lat_top.sin() - lat_bottom.sin());
    }
    area.abs()
}

fn patch_journey(tile: TileKey, block: BlockKey) -> JourneyBitmap {
    let mut bm = JourneyBitmap::new();
    let mut b = Block::new();
    for x in PATCH_LO..PATCH_HI {
        for y in PATCH_LO..PATCH_HI {
            b.set_point(x, y, true);
        }
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

fn sub(dir: &TempDir, s: &str) -> String {
    let p = dir.path().join(s);
    fs::create_dir_all(&p).unwrap();
    p.into_os_string().into_string().unwrap()
}

/// Canonical codes of every credited entity, any kind.
fn credited_codes(areas: &AreaCm2ByEntity, geo: &dyn GeoLookup) -> BTreeSet<String> {
    areas
        .keys()
        .filter_map(|id| geo.entity(*id))
        .map(|e| e.canonical_code.clone())
        .collect()
}

/// The `Country`-kind entity for a leaf: the leaf itself when it is a country,
/// otherwise the country among its ancestors. Cities sit inside provinces, so
/// the raster leaf under one is a province and its country is one level up.
fn country_of(geo: &GeoIndex, leaf: GeoEntityId) -> Option<&geo_data_format::GeoEntity> {
    std::iter::once(leaf)
        .chain(geo.ancestors(leaf))
        .filter_map(|id| geo.entity(id))
        .find(|e| e.kind == GeoEntityKind::Country)
}

/// ISO codes of all visited entities of kind Country in one layer's areas.
fn visited_country_isos(
    areas: &HashMap<GeoEntityId, u64>,
    geo: &dyn GeoLookup,
) -> BTreeSet<String> {
    areas
        .keys()
        .filter_map(|id| geo.entity(*id))
        .filter(|e| e.kind == GeoEntityKind::Country)
        .map(|e| e.canonical_code.clone())
        .collect()
}

#[test]
fn region_api_reports_correct_countries_and_areas() {
    let asset = test_utils::geo_asset("iso");
    let geo_bytes = fs::read(&asset).unwrap();
    let geo_index = GeoIndex::from_bytes(&geo_bytes).unwrap();

    // Resolve each city to its block and confirm the asset agrees with truth, so
    // any later mis-attribution is the region API's doing, not bad placement.
    let mut placements = Vec::new(); // (city_idx, tile, block, expected_area)
    for (i, c) in CITIES.iter().enumerate() {
        let (tile, block) = block_of(c.lng, c.lat);
        let id = geo_index
            .entity_of_block(tile, block)
            .unwrap_or_else(|| panic!("{}: resolved to ocean", c.name));
        let country = country_of(&geo_index, id).unwrap_or_else(|| {
            panic!(
                "{}: leaf `{}` has no country ancestor",
                c.name,
                geo_index.entity(id).unwrap().canonical_code
            )
        });
        assert_eq!(country.canonical_code, c.iso, "{}", c.name);
        placements.push((i, tile, block, expected_patch_area(tile, block)));
    }

    // Real storage with the real worldview asset; one journey per city.
    let dir = TempDir::new("region_ground_truth").unwrap();
    let storage = Storage::init(
        sub(&dir, "t"),
        sub(&dir, "d"),
        sub(&dir, "s"),
        sub(&dir, "c"),
    )
    .unwrap();
    storage
        .init_or_change_geo_data(Worldview::Iso, &geo_bytes)
        .unwrap();
    for (i, tile, block, _) in &placements {
        insert(
            &storage,
            (*i as u32) + 1,
            CITIES[*i].kind,
            patch_journey(*tile, *block),
        );
    }

    use AchievementLayer::*;

    // The two scopes this test renders: every country, and every ancestor of a
    // placement (for the rollup check). Both come from the geo tree, not coverage.
    let country_ids = region::level_scope(&geo_index, RegionKind::Country, None);
    let ancestor_ids: Vec<_> = placements
        .iter()
        .flat_map(|(_, tile, block, _)| {
            geo_index.ancestors(geo_index.entity_of_block(*tile, *block).unwrap())
        })
        .collect();

    // Region coverage for those scopes over one consistent snapshot.
    let (default_areas, flight_areas, all_areas, all_ancestor_areas) = storage
        .with_achievement_read(|s| {
            Ok((
                s.region_areas(Default, &country_ids)?,
                s.region_areas(Flight, &country_ids)?,
                s.region_areas(All, &country_ids)?,
                s.region_areas(All, &ancestor_ids)?,
            ))
        })
        .unwrap();

    // (1) Correct countries: exact visited sets per layer, nothing spurious.
    let truth = |k: JourneyKind| -> BTreeSet<String> {
        CITIES
            .iter()
            .filter(|c| c.kind == k)
            .map(|c| c.iso.into())
            .collect()
    };
    let all_truth: BTreeSet<String> = CITIES.iter().map(|c| c.iso.into()).collect();
    assert_eq!(
        visited_country_isos(&default_areas, &geo_index),
        truth(JourneyKind::DefaultKind)
    );
    assert_eq!(
        visited_country_isos(&flight_areas, &geo_index),
        truth(JourneyKind::Flight)
    );
    assert_eq!(visited_country_isos(&all_areas, &geo_index), all_truth);

    // Continents roll up: each visited country's ancestors are visited too.
    assert!(!ancestor_ids.is_empty(), "placements have ancestors");
    for anc in &ancestor_ids {
        assert!(
            all_ancestor_areas.contains_key(anc),
            "ancestor not rolled up"
        );
    }

    // (2) Correct areas: per-country visited m² vs the independent integral.
    let mut iso_to_id: HashMap<String, _> = HashMap::new();
    for id in geo_index.entities_of_kind(GeoEntityKind::Country) {
        if let Some(e) = geo_index.entity(*id) {
            iso_to_id.insert(e.canonical_code.clone(), *id);
        }
    }
    for (i, _t, _b, expected) in &placements {
        let c = &CITIES[*i];
        let layer_areas = if c.kind == JourneyKind::Flight {
            &flight_areas
        } else {
            &default_areas
        };
        let api = layer_areas[&iso_to_id[c.iso]] as f64;
        let rel = (api - expected).abs() / expected;
        assert!(
            rel < 0.02,
            "{}: area off {:.2}% (api {api:.0} vs exact {expected:.0})",
            c.name,
            rel * 100.0
        );
    }

    // (3) cos²(latitude) law: a fixed N-bit patch is N conformal Mercator pixels,
    //     which shrink in BOTH dimensions by cos(lat), so ground area scales as
    //     cos²(lat). Equatorial Nairobi vs high-latitude Reykjavik must obey it.
    let nairobi = default_areas[&iso_to_id["KEN"]] as f64;
    let reykjavik = default_areas[&iso_to_id["ISL"]] as f64;
    let cos_ratio = 64.1466f64.to_radians().cos() / 1.2921f64.to_radians().cos();
    let predicted = cos_ratio * cos_ratio;
    let measured = reykjavik / nairobi;
    assert!(
        (measured - predicted).abs() / predicted < 0.02,
        "cos²(lat) area law violated: {measured:.4} vs {predicted:.4}"
    );

    // (4) Public read API surface on real geometry: levels, a level view, and a
    //     detail for France must agree with the raw states.
    storage
        .with_achievement_read(|store| {
            let fr = iso_to_id["FRA"];
            // `region_areas` takes `&mut store`, so fetch every area first with
            // per-statement `geo()` borrows, then hold one for the assertions.
            let continent = {
                let geo = store.geo().unwrap();
                geo_index
                    .ancestors(fr)
                    .into_iter()
                    .find(|a| geo.entity(*a).map(|e| e.kind) == Some(GeoEntityKind::Continent))
                    .expect("FR has a continent ancestor")
            };
            let ids = region::level_scope(store.geo()?, RegionKind::Country, Some(continent));
            let areas = store.region_areas(All, &ids)?;
            let detail_ids = region::detail_scope(store.geo()?, fr);
            let detail_areas = store.region_areas(Default, &detail_ids)?;

            let geo = store.geo().unwrap();

            // The asset has the full world: many countries, at least one continent.
            let levels = region_levels(geo);
            let country_total = levels[&RegionKind::Country].region_count;
            assert!(
                country_total > 100,
                "expected a full world, got {country_total} countries"
            );
            assert!(levels.contains_key(&RegionKind::Continent));

            // France is listed and visited among its continent's countries.
            let view = region::level_view(geo, RegionKind::Country, &ids, &areas);
            let fr_entry = view.entries.get(&fr).expect("FR listed");
            assert!(fr_entry.visited_area_m2 > 0, "FR should be visited");
            assert!(view.visited_count >= 1 && view.visited_count <= view.region_count);

            // Detail of France (Default layer): area matches the raw coverage read.
            let detail = region::detail_view(geo, fr, &detail_areas).unwrap();
            assert_eq!(detail.entity_id, fr);
            assert_eq!(detail.node.visited_area_m2, default_areas[&fr]);
            Ok(())
        })
        .unwrap();
}

/// Past the Mercator limit (±85.0511°) `add_line` produces tile keys outside the
/// 512×512 grid: y ≥ 512 in the south, and in the north a negative Mercator y
/// wrapped through `as u16` to ~65461. Their raw `x*512 + y` is not a safe index
/// — with the real asset a track at 88°N resolves to tile (383,437), which is
/// `Single(ATA)`, so the North Pole gets credited to Antarctica, and a track at
/// 88°N/120°E indexes past the tile grid entirely.
#[test]
fn track_past_the_mercator_limit_credits_nothing() {
    let geo = GeoIndex::from_bytes(&fs::read(test_utils::geo_asset("iso")).unwrap()).unwrap();

    for (lng, lat) in [(0.0, 88.0), (120.0, 88.0), (0.0, -87.0)] {
        let mut bitmap = JourneyBitmap::new();
        bitmap.add_line(lng, lat, lng + 0.01, lat + 0.01);
        let areas = attribution::attribute(&bitmap, &geo);
        let credited = credited_codes(&areas, &geo);
        assert!(credited.is_empty(), "({lng}, {lat}) credited {credited:?}");
    }
}
