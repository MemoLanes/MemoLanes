use std::fs;

use chrono::Utc;
use memolanes_core::{
    api::api,
    gps::{ExtendedRawGPSPoint, Point, RawGPSPoint},
};
use rusqlite::Connection;
use tempdir::TempDir;

const BASE: i64 = 1_700_000_000_000;

fn point(seconds: i64, longitude: f64) -> ExtendedRawGPSPoint {
    ExtendedRawGPSPoint {
        raw_gps_point: RawGPSPoint {
            point: Point {
                latitude: 0.0,
                longitude,
            },
            timestamp_ms: Some(BASE + seconds * 1000),
            accuracy: Some(1.0),
            altitude: None,
            speed: None,
        },
        received_timestamp_ms: Utc::now().timestamp_millis(),
    }
}

#[test]
fn recording_time_gate_and_atomic_retries_share_one_ingest_path() {
    let dir = TempDir::new("api-recording").unwrap();
    let sub = |name: &str| {
        let path = dir.path().join(name);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };
    let support = sub("support");
    api::init(sub("temp"), sub("doc"), support.clone(), sub("cache")).unwrap();
    api::toggle_raw_data_mode(true);
    let connection = Connection::open(dir.path().join("support/main.db")).unwrap();
    let counts = || -> (i64, i64) {
        connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM ongoing_journey),
                        (SELECT COUNT(*) FROM ongoing_journey_raw_data)",
                (),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    };
    let cursor = || -> i64 {
        connection
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM setting WHERE key='RECORDING_LAST_TIMESTAMP_MS'",
                (),
                |row| row.get(0),
            )
            .unwrap()
    };

    assert!(api::on_location_update(point(1, 0.0)).unwrap());
    assert!(api::on_location_update(point(20, 0.00002)).unwrap());
    assert!(!api::on_location_update(point(20, 0.1)).unwrap());
    assert!(!api::on_location_update(point(10, 0.2)).unwrap());
    assert_eq!(counts(), (2, 2));
    assert_eq!(cursor(), BASE + 20_000);

    connection
        .execute_batch(
            "CREATE TRIGGER fail_recording_cursor BEFORE INSERT ON setting
             WHEN NEW.key = 'RECORDING_LAST_TIMESTAMP_MS'
             BEGIN SELECT RAISE(ABORT, 'injected cursor failure'); END;",
        )
        .unwrap();
    let next = point(30, 0.00003);
    assert!(api::on_location_update(next.clone()).is_err());
    assert_eq!(counts(), (2, 2));
    assert_eq!(cursor(), BASE + 20_000);
    connection
        .execute_batch("DROP TRIGGER fail_recording_cursor;")
        .unwrap();
    assert!(api::on_location_update(next).unwrap());
    assert_eq!(counts(), (3, 3));

    let mut inaccurate = point(40, 0.00004);
    inaccurate.raw_gps_point.accuracy = Some(100.0);
    assert!(!api::on_location_update(inaccurate.clone()).unwrap());
    assert_eq!(cursor(), BASE + 30_000);
    assert_eq!(counts(), (3, 4));
    assert!(!api::on_location_update(inaccurate).unwrap());
    assert_eq!(counts(), (3, 5));
    assert_eq!(cursor(), BASE + 30_000);
    assert!(api::on_location_update(point(35, 0.00004)).unwrap());
    assert_eq!(cursor(), BASE + 35_000);
    assert_eq!(counts(), (4, 6));

    assert!(api::finalize_ongoing_journey().unwrap());
    assert!(!api::on_location_update(point(20, 0.2)).unwrap());
    assert!(!api::on_location_update(point(35, 0.2)).unwrap());
    assert!(!api::has_ongoing_journey().unwrap());
    assert_eq!(counts(), (0, 0));
    assert_eq!(api::list_all_journeys().unwrap().len(), 1);
    assert!(api::on_location_update(point(50, 0.00005)).unwrap());
    assert_eq!(cursor(), BASE + 50_000);

    // Short gaps use capture time, even though the cache is drained much later.
    assert_eq!(
        api::try_auto_finalize_before_location(point(60, 0.00006)).unwrap(),
        Some(false)
    );
    assert_eq!(
        api::try_auto_finalize_before_location(point(50, 0.00006)).unwrap(),
        None
    );
    assert!(api::on_location_update(point(60, 0.00006)).unwrap());

    let next_day = point(86_400, 0.0001);
    let mut inaccurate = next_day.clone();
    inaccurate.raw_gps_point.accuracy = Some(100.0);
    assert_eq!(
        api::try_auto_finalize_before_location(inaccurate).unwrap(),
        None
    );
    assert_eq!(
        api::try_auto_finalize_before_location(next_day.clone()).unwrap(),
        Some(true)
    );
    assert_eq!(api::list_all_journeys().unwrap().len(), 2);
    assert_eq!(counts(), (0, 0));
    assert!(api::on_location_update(next_day).unwrap());
    assert_eq!(counts(), (1, 1));
}
