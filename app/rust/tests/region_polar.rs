#![cfg(geo_bins_present)]
//! Real-source polar achievement coverage, including the Mercator seam.

pub mod test_utils;

use bitmap_geometry::{polar_pixel_area_cm2, MERCATOR_MAX_LATITUDE};
use chrono::NaiveDate;
use geo_data_format::{GeoEntityId, GeoEntityKind, Worldview};
use memolanes_core::{
    achievement::{
        attribution,
        layer::AchievementLayer,
        region::{self, RegionKind},
    },
    geo::{GeoIndex, GeoLookup},
    geo_provenance::bundled_provenance_hash,
    journey_bitmap::{
        area::{journey_bitmap_area_cm2, journey_bitmap_area_m2_rounded},
        JourneyBitmap,
    },
    journey_data::JourneyData,
    journey_header::JourneyKind,
    main_db::NewJourney,
    storage::Storage,
};
use std::{collections::HashMap, fs};

fn antarctica(geo: &GeoIndex) -> (GeoEntityId, GeoEntityId) {
    let countries = geo
        .describe(geo.entities_of_kind(GeoEntityKind::Admin0))
        .unwrap();
    let (&country, entity) = countries
        .iter()
        .find(|(_, e)| e.canonical_code == "ATA")
        .unwrap();
    let continent = entity.parent_id.unwrap();
    assert_eq!(
        geo.describe(&[continent]).unwrap()[&continent].canonical_code,
        "AN"
    );
    (country, continent)
}
fn point(lng: f64, lat: f64) -> JourneyBitmap {
    let mut b = JourneyBitmap::new();
    b.edit().add_line(lng, lat, lng, lat);
    b
}

#[test]
fn antarctica_attribution_matches_global_area_through_seams_poles_and_dateline() {
    for &worldview in Worldview::ALL {
        let geo = GeoIndex::open(&test_utils::geo_asset(worldview.spec().id)).unwrap();
        let (country, continent) = antarctica(&geo);
        for (a, b) in [
            ((0.0, -90.0), (0.0, -90.0)),
            ((-180.0, -90.0), (-180.0, -90.0)),
            ((180.0, -90.0), (180.0, -90.0)),
            ((-180.0, -90.0), (180.0, -90.0)),
            ((-180.0, -88.0), (180.0, -88.0)),
            ((0.0, -87.0), (0.01, -86.99)),
            ((45.0, -87.0), (45.01, -87.01)),
            (
                (45.0, -MERCATOR_MAX_LATITUDE + 0.001),
                (45.0, -MERCATOR_MAX_LATITUDE - 0.001),
            ),
            (
                (45.0, -MERCATOR_MAX_LATITUDE),
                (45.0, -MERCATOR_MAX_LATITUDE),
            ),
            ((179.999, -88.0), (-179.999, -88.0)),
            (
                (-180.0, -MERCATOR_MAX_LATITUDE + 0.001),
                (180.0, -MERCATOR_MAX_LATITUDE - 0.001),
            ),
            (
                (-180.0, -MERCATOR_MAX_LATITUDE),
                (180.0, -MERCATOR_MAX_LATITUDE),
            ),
        ] {
            let mut bitmap = JourneyBitmap::new();
            bitmap.edit().add_line(a.0, a.1, b.0, b.1);
            let area = journey_bitmap_area_cm2(&bitmap, None);
            assert!(area > 0);
            assert_eq!(
                attribution::attribute(&bitmap, &geo).unwrap(),
                HashMap::from([(country, area), (continent, area)]),
                "{} {a:?} {b:?}",
                worldview.spec().id
            );
        }
        let arctic_points = [-180.0, 0.0, 120.0, 179.9, 180.0]
            .into_iter()
            .flat_map(|lng| [88.0, 90.0].map(|lat| ((lng, lat), (lng, lat))));
        let arctic_tracks = [
            ((0.0, 88.0), (0.01, 88.01)),
            ((120.0, 88.0), (120.01, 88.01)),
        ];
        for (a, b) in arctic_points.chain(arctic_tracks) {
            let mut bitmap = JourneyBitmap::new();
            bitmap.edit().add_line(a.0, a.1, b.0, b.1);
            assert!(journey_bitmap_area_cm2(&bitmap, None) > 0);
            assert!(
                attribution::attribute(&bitmap, &geo).unwrap().is_empty(),
                "{} Arctic {a:?} {b:?}",
                worldview.spec().id
            );
        }
        let mut poles = point(-180.0, -90.0);
        for (lng, lat) in [(180.0, -90.0), (-180.0, 90.0), (180.0, 90.0)] {
            poles.edit().merge(point(lng, lat));
        }
        let one_pole_cm2 = 81 * polar_pixel_area_cm2();
        assert_eq!(journey_bitmap_area_cm2(&poles, None), 2 * one_pole_cm2);
        assert_eq!(
            attribution::attribute(&poles, &geo).unwrap(),
            HashMap::from([(country, one_pole_cm2), (continent, one_pole_cm2)]),
        );
        let totals = geo.describe(&[country, continent]).unwrap();
        assert!(totals[&country].total_area_m2 > 11_000_000_000_000);
        assert!(totals[&continent].total_area_m2 >= totals[&country].total_area_m2);
    }
}

#[test]
fn polar_visited_status_layers_and_union_survive_warm_and_cold_cache_reads() {
    for &worldview in Worldview::ALL {
        let asset = test_utils::geo_asset(worldview.spec().id);
        let geo = GeoIndex::open(&asset).unwrap();
        let (country, continent) = antarctica(&geo);
        let dir = tempdir::TempDir::new("region_polar").unwrap();
        let sub = |s: &str| {
            let path = dir.path().join(s);
            fs::create_dir_all(&path).unwrap();
            path.to_str().unwrap().to_owned()
        };
        let storage = Storage::init(sub("t"), sub("d"), sub("s"), sub("c")).unwrap();
        storage
            .init_or_change_geo_data(
                worldview,
                bundled_provenance_hash(worldview).unwrap(),
                &fs::read(asset).unwrap(),
            )
            .unwrap();
        let mut expected_default = JourneyBitmap::new();
        let mut expected_flight = JourneyBitmap::new();
        let mut expected_all = JourneyBitmap::new();
        // Prime empty layer caches before incremental inserts.
        storage
            .with_achievement_read(|reader| {
                for layer in AchievementLayer::ALL_LAYERS {
                    assert_eq!(reader.explored_area_m2(layer)?, 0);
                }
                Ok(())
            })
            .unwrap();
        for (step, (kind, bitmap)) in [
            (JourneyKind::Flight, point(-180.0, 90.0)), // ocean-only must not mark Antarctica visited
            (JourneyKind::Flight, point(180.0, 90.0)),  // same North Pole, no additional area
            (JourneyKind::DefaultKind, point(-180.0, -90.0)),
            (JourneyKind::Flight, point(180.0, -90.0)), // South Pole overlaps across layers
            (JourneyKind::DefaultKind, point(180.0, -90.0)), // same South Pole, no additional area
            (
                JourneyKind::DefaultKind,
                point(45.0, -MERCATOR_MAX_LATITUDE),
            ),
            (
                JourneyKind::DefaultKind,
                point(-180.0, -MERCATOR_MAX_LATITUDE),
            ),
            (
                JourneyKind::DefaultKind,
                point(180.0, -MERCATOR_MAX_LATITUDE),
            ),
            (JourneyKind::Flight, point(-180.0, -88.0)),
            (JourneyKind::Flight, point(180.0, -88.0)),
            (JourneyKind::DefaultKind, point(0.0, -90.0)), // duplicate journey
        ]
        .into_iter()
        .enumerate()
        {
            if kind == JourneyKind::Flight {
                expected_flight.edit().merge(bitmap.clone());
            } else {
                expected_default.edit().merge(bitmap.clone());
            }
            expected_all.edit().merge(bitmap.clone());
            storage
                .with_db_txn(|txn| {
                    txn.create_and_insert_journey(NewJourney {
                        journey_date: NaiveDate::from_ymd_opt(2025, 1, step as u32 + 1).unwrap(),
                        start: None,
                        end: None,
                        created_at: None,
                        journey_kind: kind,
                        note: None,
                        journey_data: JourneyData::Bitmap(bitmap),
                        raw_data: None,
                    })
                })
                .unwrap();
            for cold in [false, true] {
                if cold {
                    storage.clear_all_cache().unwrap();
                }
                storage
                    .with_achievement_read(|reader| {
                        for (layer, expected) in [
                            (AchievementLayer::Default, &expected_default),
                            (AchievementLayer::Flight, &expected_flight),
                            (AchievementLayer::All, &expected_all),
                        ] {
                            let areas = reader.region_areas(layer, &[country, continent])?;
                            let cm2 = attribution::attribute(expected, &geo).unwrap();
                            for id in [country, continent] {
                                assert_eq!(
                                    areas.get(&id).copied().unwrap_or(0),
                                    ((cm2.get(&id).copied().unwrap_or(0) + 5000) / 10000) as u64,
                                    "{} step {step} {layer:?} cold={cold}",
                                    worldview.spec().id
                                );
                            }
                            assert_eq!(
                                reader.explored_area_m2(layer)?,
                                journey_bitmap_area_m2_rounded(expected, None)
                            );
                            let view = region::level_view(
                                reader.geo()?,
                                RegionKind::Admin0,
                                &[country],
                                &areas,
                            )?;
                            assert_eq!(view.visited_count, u32::from(cm2.contains_key(&country)));
                            assert_eq!(
                                view.entries[&country].visited_area_m2 > 0,
                                cm2.contains_key(&country)
                            );
                            assert!(view.entries[&country].total_area_m2 > 11_000_000_000_000);
                            let continents = region::level_view(
                                reader.geo()?,
                                RegionKind::Continent,
                                &[continent],
                                &areas,
                            )?;
                            assert_eq!(continents.visited_count, view.visited_count);
                            assert_eq!(
                                continents.entries[&continent].visited_area_m2,
                                view.entries[&country].visited_area_m2,
                            );
                        }
                        Ok(())
                    })
                    .unwrap();
            }
        }
    }
}
