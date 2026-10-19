pub mod test_utils;
use crate::test_utils::{draw_line1, draw_line2};
use chrono::NaiveDate;
use memolanes_core::main_db::NewJourney;
use memolanes_core::{
    achievement::layer::AchievementLayer,
    achievement::on_demand::explored_areas_from_snapshot,
    journey_bitmap::area::{
        cm2_to_m2_rounded, journey_bitmap_area_cm2, journey_bitmap_area_m2_rounded,
    },
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    storage::Storage,
};
use std::collections::HashMap;
use std::fs;
use tempdir::TempDir;

fn setup_storage_for_test<F>(f: F)
where
    F: FnOnce(Storage),
{
    let temp_dir = TempDir::new("test_achievement_area").unwrap();
    let sub_folder = |sub| {
        let path = temp_dir.path().join(sub);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };
    let storage = Storage::init(
        sub_folder("temp/"),
        sub_folder("doc/"),
        sub_folder("support/"),
        sub_folder("cache/"),
    )
    .unwrap();
    f(storage);
}

fn explored_areas(
    storage: &Storage,
    layers: &[AchievementLayer],
) -> HashMap<AchievementLayer, u64> {
    storage
        .with_journey_snapshot(|snapshot| explored_areas_from_snapshot(snapshot, layers))
        .unwrap()
}

/// Direct computation of a layer's explored area — the oracle
/// `explored_areas_from_snapshot` must reproduce.
fn oracle_area(storage: &Storage, layer: AchievementLayer) -> u64 {
    storage
        .with_journey_snapshot(|snapshot| {
            let bitmap = snapshot.finalized_bitmap(&layer.to_layer_kind(), None)?;
            Ok(journey_bitmap_area_m2_rounded(&bitmap, None))
        })
        .unwrap()
}

#[test]
fn explored_areas_match_direct_fold() {
    setup_storage_for_test(|storage| {
        let layers = AchievementLayer::ALL_LAYERS;

        // Empty storage: every layer maps to zero.
        let empty = explored_areas(&storage, &layers);
        assert_eq!(empty.len(), 3);
        assert_eq!(empty[&AchievementLayer::Default], 0);
        assert_eq!(empty[&AchievementLayer::Flight], 0);
        assert_eq!(empty[&AchievementLayer::All], 0);

        // A Default journey and a Flight journey covering different lines.
        let mut default_bitmap = JourneyBitmap::new();
        draw_line1(&mut default_bitmap);
        let mut flight_bitmap = JourneyBitmap::new();
        draw_line2(&mut flight_bitmap);

        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(NewJourney {
                    journey_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    start: None,
                    end: None,
                    created_at: None,
                    journey_kind: JourneyKind::DefaultKind,
                    note: None,
                    journey_data: JourneyData::Bitmap(default_bitmap.clone()),
                    raw_data: None,
                })
            })
            .unwrap();
        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(NewJourney {
                    journey_date: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                    start: None,
                    end: None,
                    created_at: None,
                    journey_kind: JourneyKind::Flight,
                    note: None,
                    journey_data: JourneyData::Bitmap(flight_bitmap.clone()),
                    raw_data: None,
                })
            })
            .unwrap();

        // Map result equals the direct fold for every layer.
        let map = explored_areas(&storage, &layers);
        for layer in layers {
            assert_eq!(map[&layer], oracle_area(&storage, layer), "layer {layer:?}");
        }

        // Non-empty; `All` is a superset of each component layer.
        let default = map[&AchievementLayer::Default];
        let flight = map[&AchievementLayer::Flight];
        let all = map[&AchievementLayer::All];
        assert!(default > 0);
        assert!(flight > 0);
        assert!(all >= default && all >= flight);
    });
}

#[test]
fn polar_area_is_counted_once_across_layers_in_cached_and_on_demand_reads() {
    setup_storage_for_test(|storage| {
        let layers = AchievementLayer::ALL_LAYERS;
        let read_cached = || {
            storage
                .with_achievement_read(|reader| {
                    layers
                        .into_iter()
                        .map(|layer| Ok((layer, reader.explored_area_m2(layer)?)))
                        .collect::<anyhow::Result<HashMap<_, _>>>()
                })
                .unwrap()
        };
        // Warm all three layers before inserting anything, so subsequent reads
        // exercise incremental updates of the persisted totals as well.
        assert!(read_cached().values().all(|area| *area == 0));

        let mut mercator = JourneyBitmap::new();
        mercator.edit().add_line(0.0, 0.0, 0.0, 0.0);
        let mercator_cm2 = journey_bitmap_area_cm2(&mercator, None);
        // A pole point's complete 9×9 brush lies inside the equal-area disk.
        let pole_cm2 = 81 * 11010;
        let mut default_cm2 = 0;
        let mut flight_cm2 = 0;
        let mut all_cm2 = 0;

        for (step, (kind, latitudes)) in [
            (JourneyKind::DefaultKind, &[0.0, 90.0][..]),
            (JourneyKind::Flight, &[90.0, -90.0][..]),
            (JourneyKind::DefaultKind, &[90.0][..]),
        ]
        .into_iter()
        .enumerate()
        {
            let vector = JourneyVector {
                track_segments: latitudes
                    .iter()
                    .map(|&latitude| TrackSegment {
                        track_points: vec![TrackPoint {
                            longitude: 0.0,
                            latitude,
                        }],
                    })
                    .collect(),
            };
            storage
                .with_db_txn(|txn| {
                    txn.create_and_insert_journey(NewJourney {
                        journey_date: NaiveDate::from_ymd_opt(2025, 1, step as u32 + 1).unwrap(),
                        start: None,
                        end: None,
                        created_at: None,
                        journey_kind: kind,
                        note: None,
                        journey_data: JourneyData::Vector(vector),
                        raw_data: None,
                    })
                })
                .unwrap();
            if step == 0 {
                default_cm2 = mercator_cm2 + pole_cm2;
                all_cm2 = default_cm2;
            } else if step == 1 {
                flight_cm2 = 2 * pole_cm2;
                all_cm2 += pole_cm2; // North is already covered by Default.
            }
            let expected = HashMap::from([
                (AchievementLayer::Default, cm2_to_m2_rounded(default_cm2)),
                (AchievementLayer::Flight, cm2_to_m2_rounded(flight_cm2)),
                (AchievementLayer::All, cm2_to_m2_rounded(all_cm2)),
            ]);
            assert_eq!(read_cached(), expected, "cached step {step}");
            assert_eq!(
                explored_areas(&storage, &layers),
                expected,
                "on-demand step {step}"
            );
        }
    });
}
