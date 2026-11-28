use chrono::{Duration, Local, TimeZone};
use memolanes_core::{
    api::api,
    gps::{ExtendedRawGPSPoint, Point, RawGPSPoint},
    gps_processor::ProcessResult,
    journey_data::JourneyData,
    main_db::MainDb,
};
use tempdir::TempDir;

#[test]
fn recording_boundaries_follow_sample_time() {
    let dir = TempDir::new("recording-auto-finalize").unwrap();
    let folder = dir.path().to_str().unwrap();
    let start = Local.with_ymd_and_hms(2025, 1, 15, 23, 50, 0).unwrap();
    let sample = |minutes: i64, delayed: bool| {
        let at = (start + Duration::minutes(minutes)).timestamp_millis();
        ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                timestamp_ms: Some(at),
                point: Point {
                    latitude: 31.0 + minutes as f64 / 10_000.0,
                    longitude: 121.0,
                },
                altitude: None,
                speed: None,
                accuracy: None,
            },
            received_timestamp_ms: if delayed {
                (start + Duration::days(2)).timestamp_millis()
            } else {
                at
            },
        }
    };

    // Existing recording spans 26 hours: its restored idle threshold is two minutes.
    let mut db = MainDb::open(folder).unwrap();
    for minutes in [-26 * 60, -1] {
        db.record(&sample(minutes, false).raw_gps_point, ProcessResult::Append)
            .unwrap();
    }
    drop(db);
    api::init(folder.into(), folder.into(), folder.into(), folder.into()).unwrap();
    let finalizes_previous = |fix| {
        api::on_location_update(fix)
            .unwrap()
            .previous_journey_finalized
    };

    assert!(api::has_ongoing_journey().unwrap());
    assert!(!finalizes_previous(sample(0, false)));
    assert!(finalizes_previous(sample(2, false)));
    assert!(api::try_auto_finalize_journey().unwrap());
    assert!(!api::has_ongoing_journey().unwrap());
    api::delete_all_journeys().unwrap();

    // Same samples in a running process, delivered live or together two days later.
    for delayed in [false, true] {
        assert!(!finalizes_previous(sample(0, delayed)));
        for minutes in [5, 15] {
            assert!(!finalizes_previous(sample(minutes, delayed)));
        }
        let mut ignored = sample(34, delayed);
        ignored.raw_gps_point.accuracy = Some(1000.0);
        assert!(!finalizes_previous(ignored));
        assert!(finalizes_previous(sample(35, delayed)));
        // An older fix in a later batch must not move the new journey's tail backwards.
        assert!(!finalizes_previous(sample(10, delayed)));
        assert!(!finalizes_previous(sample(36, delayed)));
        assert!(api::finalize_ongoing_journey().unwrap());

        let mut headers = api::list_all_journeys().unwrap();
        headers.sort_by_key(|h| h.start);
        assert_eq!(headers.len(), 2);
        for (header, minutes) in headers.iter().zip([&[0, 5, 15][..], &[35, 36]]) {
            assert_eq!(header.start.unwrap(), start + Duration::minutes(minutes[0]));
            assert_eq!(
                header.end.unwrap(),
                start + Duration::minutes(*minutes.last().unwrap())
            );
            let data = MainDb::open(folder)
                .unwrap()
                .with_txn(|txn| txn.get_journey_data(&header.id))
                .unwrap();
            let JourneyData::Vector(vector) = data else {
                panic!("expected recorded vector")
            };
            let recorded_minutes: Vec<_> = vector
                .track_segments
                .iter()
                .flat_map(|segment| &segment.track_points)
                .map(|p| ((p.latitude - 31.0) * 10_000.0).round() as i64)
                .collect();
            assert_eq!(recorded_minutes, minutes);
        }
        api::delete_all_journeys().unwrap();
    }
}
