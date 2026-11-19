use chrono::{Local, TimeZone};
use memolanes_core::{
    api::api::{self, LayerFilter},
    gps::{ExtendedRawGPSPoint, Point, RawGPSPoint},
    journey_bitmap::JourneyBitmap,
};
use tempdir::TempDir;

fn journey(day: u32, hour: u32, longitude: f64) -> Vec<RawGPSPoint> {
    let start = Local
        .with_ymd_and_hms(2026, 8, day, hour, 0, 0)
        .single()
        .unwrap()
        .timestamp_millis();
    (0..3)
        .map(|i| RawGPSPoint {
            timestamp_ms: Some(start + i * 1000),
            point: Point {
                latitude: 0.0,
                longitude: longitude + i as f64 * 0.0001,
            },
            accuracy: Some(1.0),
            altitude: None,
            speed: None,
        })
        .collect()
}

fn updates(points: &[RawGPSPoint]) -> Vec<ExtendedRawGPSPoint> {
    points
        .iter()
        .map(|point| ExtendedRawGPSPoint {
            raw_gps_point: point.clone(),
            // All locations are delivered long after collection, on one receive
            // timestamp. Only their provider timestamps may determine boundaries.
            received_timestamp_ms: Local
                .with_ymd_and_hms(2026, 9, 1, 12, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis(),
        })
        .collect()
}

fn bitmap(journeys: &[Vec<RawGPSPoint>]) -> JourneyBitmap {
    let mut bitmap = JourneyBitmap::new();
    for journey in journeys {
        for pair in journey.windows(2) {
            bitmap
                .edit()
                .add_line(pair[0].point.longitude, 0.0, pair[1].point.longitude, 0.0);
        }
    }
    bitmap
}

fn assert_map(expected: JourneyBitmap) {
    let state = api::for_testing::get_main_map_state();
    let state = state.lock().unwrap();
    assert_eq!(state.map_renderer.peek_latest_bitmap(), &expected);
}

#[test]
fn delayed_batches_finalize_each_journey_and_keep_the_map_consistent() {
    let dir = TempDir::new("api-recording").unwrap();
    let sub = |name| {
        let path = dir.path().join(name);
        std::fs::create_dir(&path).unwrap();
        path.to_string_lossy().into_owned()
    };
    api::init(sub("temp"), sub("doc"), sub("support"), sub("cache")).unwrap();
    let cache = rusqlite::Connection::open(dir.path().join("cache/cache.db")).unwrap();
    cache
        .execute_batch(
            "CREATE TRIGGER fail_cache_write BEFORE INSERT ON journey_cache__full
             BEGIN SELECT RAISE(ABORT, 'injected cache write failure'); END;",
        )
        .unwrap();
    let mut journeys = vec![
        journey(11, 8, 0.0),
        journey(11, 15, 0.1),
        journey(11, 22, 0.2),
    ];
    let points: Vec<_> = journeys.iter().flatten().cloned().collect();
    assert!(api::on_location_updates(updates(&points)).unwrap());
    assert_eq!(api::list_all_journeys().unwrap().len(), 2);
    assert!(api::has_ongoing_journey().unwrap());
    assert_map(bitmap(&journeys));
    cache
        .execute_batch("DROP TRIGGER fail_cache_write")
        .unwrap();

    api::set_main_map_layer_filter(&LayerFilter {
        current_journey: false,
        default_kind: true,
        flight_kind: false,
    })
    .unwrap();
    assert_map(bitmap(&journeys[..2]));
    api::set_main_map_layer_filter(&LayerFilter {
        current_journey: true,
        default_kind: true,
        flight_kind: false,
    })
    .unwrap();

    api::free_resource_for_long_time_background();
    journeys.push(journey(12, 5, 0.3));
    assert!(api::on_location_updates(updates(journeys.last().unwrap())).unwrap());
    assert_map(JourneyBitmap::new());
    api::reload_resource_for_foreground().unwrap();
    assert_map(bitmap(&journeys));
    assert!(api::finalize_ongoing_journey().unwrap());
    assert_eq!(api::list_all_journeys().unwrap().len(), 4);
    assert!(!api::try_finalize_paused_journey().unwrap());
    assert_map(bitmap(&journeys));
}
