pub mod test_utils;
use crate::test_utils::{draw_line1, draw_line2};
use chrono::NaiveDate;
use memolanes_core::{
    achievement::layer::AchievementLayer, achievement::on_demand::explored_areas_from_snapshot,
    journey_area_utils::journey_bitmap_area_m2_rounded, journey_bitmap::JourneyBitmap,
    journey_data::JourneyData, journey_header::JourneyKind, storage::Storage,
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
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::DefaultKind,
                    None,
                    JourneyData::Bitmap(default_bitmap.clone()),
                )
            })
            .unwrap();
        storage
            .with_db_txn(|txn| {
                txn.create_and_insert_journey(
                    NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
                    None,
                    None,
                    None,
                    JourneyKind::Flight,
                    None,
                    JourneyData::Bitmap(flight_bitmap.clone()),
                )
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
