use super::*;
use crate::gps::RawGPSPoint;
use crate::main_db::MainDb;
use chrono::{TimeZone, Utc};
use tempdir::TempDir;

fn delivered(location: &RawGPSPoint) -> ExtendedRawGPSPoint {
    ExtendedRawGPSPoint {
        raw_gps_point: location.clone(),
        received_timestamp_ms: location.timestamp_ms.unwrap_or_default(),
    }
}

struct Recorder {
    dir: TempDir,
    db: MainDb,
    processor: GpsPreprocessor,
}

impl Recorder {
    fn new() -> Self {
        let dir = TempDir::new("recording-boundaries").unwrap();
        let db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
        Self {
            dir,
            db,
            processor: GpsPreprocessor::new(),
        }
    }

    fn record(&mut self, location: &RawGPSPoint) -> RecordedLocation {
        let mut next = self.processor.clone();
        let result = self
            .db
            .with_txn(|txn| record_location(txn, &mut next, &delivered(location), false))
            .unwrap();
        self.processor = next;
        result
    }

    fn restart(&mut self) {
        self.db = MainDb::open(self.dir.path().to_str().unwrap()).unwrap();
        self.processor = GpsPreprocessor::new();
    }

    fn ranges(&mut self) -> Vec<(i64, i64)> {
        self.db
            .with_txn(|txn| {
                let mut ranges: Vec<_> = txn
                    .query_journeys(None, None, None)?
                    .into_iter()
                    .map(|journey| {
                        (
                            journey.start.unwrap().timestamp(),
                            journey.end.unwrap().timestamp(),
                        )
                    })
                    .collect();
                ranges.sort_unstable();
                Ok(ranges)
            })
            .unwrap()
    }

    fn ongoing(&mut self) -> Option<(i64, i64)> {
        self.db
            .with_txn(|txn| {
                Ok(txn
                    .get_ongoing_journey_timestamp_range()?
                    .map(|(start, end)| (start.timestamp(), end.timestamp())))
            })
            .unwrap()
    }
}

fn point(hour: u32, minute: u32, longitude: f64) -> RawGPSPoint {
    RawGPSPoint {
        timestamp_ms: Some(
            Local
                .with_ymd_and_hms(2026, 8, 11, hour, minute, 0)
                .single()
                .unwrap()
                .timestamp_millis(),
        ),
        point: Point {
            latitude: 0.0,
            longitude,
        },
        accuracy: Some(1.0),
        altitude: None,
        speed: None,
    }
}

fn two_journeys() -> Vec<RawGPSPoint> {
    vec![
        point(8, 0, 0.0),
        point(8, 1, 0.0001),
        point(8, 2, 0.0002),
        point(15, 0, 0.1),
        point(15, 1, 0.1001),
        point(15, 2, 0.1002),
    ]
}

fn range(points: &[RawGPSPoint]) -> (i64, i64) {
    (
        points.first().unwrap().timestamp_ms.unwrap() / 1000,
        points.last().unwrap().timestamp_ms.unwrap() / 1000,
    )
}

#[test]
fn splits_before_b1_inside_a_delayed_batch() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    let results: Vec<_> = points.iter().map(|point| recorder.record(point)).collect();
    assert_eq!(results.iter().filter(|r| r.journey_finalized).count(), 1);
    assert!(results[3].journey_finalized);
    assert_eq!(results[3].process_result, ProcessResult::NewSegment);
    assert!(results[3].previous_point.is_none());
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
    assert_eq!(recorder.ongoing(), Some(range(&points[3..])));
}

#[test]
fn raw_data_v2_stays_with_each_journey_when_a_batch_is_replayed() {
    let mut recorder = Recorder::new();
    let points: Vec<_> = two_journeys().iter().map(delivered).collect();
    recorder
        .db
        .with_txn(|txn| {
            for point in points.iter().chain(&points) {
                record_location(txn, &mut recorder.processor, point, true)?;
            }

            let finalized = txn.query_journeys(None, None, None)?;
            assert_eq!(finalized.len(), 1);
            let raw = txn
                .get_journey_raw_data(&finalized[0].id)?
                .unwrap()
                .deserialize()?;
            assert_eq!(raw.points, points[..3]);
            assert_eq!(txn.get_ongoing_journey_raw_data()?.points, points[3..]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn boundaries_survive_restart_at_every_possible_batch_boundary() {
    let points = two_journeys();
    for boundaries in 0..(1 << (points.len() - 1)) {
        let mut recorder = Recorder::new();
        for (i, location) in points.iter().enumerate() {
            recorder.record(location);
            if boundaries & (1 << i) != 0 {
                recorder.restart();
            }
        }
        assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
        assert_eq!(recorder.ongoing(), Some(range(&points[3..])));
    }
}

#[test]
fn replay_after_finalization_and_restart_does_not_recreate_journeys() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..4] {
        recorder.record(point);
    }
    recorder.restart();
    for point in &points {
        recorder.record(point);
    }
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
    assert_eq!(recorder.ongoing(), Some(range(&points[3..])));
    recorder
        .db
        .with_txn(|txn| txn.finalize_ongoing_journey(false))
        .unwrap();
    recorder.restart();
    for point in &points {
        assert_eq!(recorder.record(point).process_result, ProcessResult::Ignore);
    }
    assert_eq!(
        recorder.ranges(),
        vec![range(&points[..3]), range(&points[3..])]
    );
    assert_eq!(recorder.ongoing(), None);
}

#[test]
fn bad_locations_cannot_finalize_or_advance_the_delivery_cursor() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..3] {
        recorder.record(point);
    }
    let mut inaccurate = points[3].clone();
    inaccurate.accuracy = Some(100.0);
    let mut invalid_time = points[3].clone();
    invalid_time.timestamp_ms = Some(i64::MAX);
    let mut invalid_coordinate = points[3].clone();
    invalid_coordinate.point.latitude = f64::NAN;
    let mut missing_time = points[3].clone();
    missing_time.timestamp_ms = None;
    for bad in [inaccurate, invalid_time, invalid_coordinate, missing_time] {
        let result = recorder.record(&bad);
        assert!(!result.journey_finalized);
        assert_eq!(result.process_result, ProcessResult::Ignore);
    }
    assert!(recorder.ranges().is_empty());
    assert!(recorder.record(&points[3]).journey_finalized);
}

#[test]
fn valid_stationary_fixes_advance_the_idle_clock_without_extending_activity() {
    let mut recorder = Recorder::new();
    let first = point(8, 0, 0.0);
    recorder.record(&first);
    for hour in 9..14 {
        let result = recorder.record(&point(hour, 0, 0.0));
        assert_eq!(result.process_result, ProcessResult::Ignore);
        assert!(!result.journey_finalized);
    }
    let resumed = point(14, 0, 0.0);
    let result = recorder.record(&resumed);
    assert!(result.journey_finalized);
    assert_eq!(result.process_result, ProcessResult::NewSegment);
    assert_eq!(recorder.ranges(), vec![range(&[first])]);
    assert_eq!(recorder.ongoing(), Some(range(&[resumed])));
}

#[test]
fn paused_journey_can_expire_without_another_location() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..3] {
        recorder.record(point);
    }
    recorder.restart();
    let at = |hour, minute| {
        DateTime::<Utc>::from_timestamp_millis(point(hour, minute, 0.0).timestamp_ms.unwrap())
            .unwrap()
            .with_timezone(&Local)
    };
    assert!(!recorder
        .db
        .with_txn(|txn| txn.try_auto_finalize_journey_at(at(14, 1), false))
        .unwrap());
    assert!(recorder
        .db
        .with_txn(|txn| txn.try_auto_finalize_journey_at(at(14, 2), false))
        .unwrap());
    assert!(!recorder
        .db
        .with_txn(|txn| txn.try_auto_finalize_journey_at(at(15, 0), false))
        .unwrap());
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
}

#[test]
fn boundary_and_point_roll_back_together_when_persistence_fails() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..3] {
        recorder.record(point);
    }
    let mut next = recorder.processor.clone();
    let result: Result<()> = recorder.db.with_txn(|txn| {
        assert!(record_location(txn, &mut next, &delivered(&points[3]), false)?.journey_finalized);
        anyhow::bail!("injected transaction failure")
    });
    assert!(result.is_err());
    assert!(recorder.ranges().is_empty());
    assert_eq!(recorder.ongoing(), Some(range(&points[..3])));
    assert!(recorder.record(&points[3]).journey_finalized);
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
}

#[test]
fn upgrade_uses_existing_ongoing_journey_before_any_cursor_was_saved() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..3] {
        recorder.db.record(point, ProcessResult::Append).unwrap();
    }
    recorder.restart();
    assert_eq!(
        recorder.record(&points[1]).process_result,
        ProcessResult::Ignore
    );
    assert!(recorder.record(&points[3]).journey_finalized);
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
}

#[test]
fn untimed_legacy_points_do_not_hide_a_journey_or_block_new_fixes() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    let mut untimed = points[0].clone();
    untimed.timestamp_ms = None;
    recorder
        .db
        .record(&untimed, ProcessResult::NewSegment)
        .unwrap();
    assert!(recorder
        .db
        .with_txn(|txn| txn.has_ongoing_journey())
        .unwrap());
    assert_eq!(recorder.ongoing(), None);
    recorder
        .db
        .record(&points[0], ProcessResult::Append)
        .unwrap();
    recorder.db.record(&untimed, ProcessResult::Append).unwrap();
    recorder.restart();

    assert_eq!(
        recorder.record(&points[0]).process_result,
        ProcessResult::Ignore
    );
    assert_eq!(
        recorder.record(&points[1]).process_result,
        ProcessResult::NewSegment
    );
    assert_eq!(recorder.ongoing(), Some(range(&points[..2])));
}

#[test]
fn future_fix_does_not_split_the_journey_or_advance_the_cursor() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    recorder.record(&points[0]);
    let mut future = delivered(&points[1]);
    future.raw_gps_point.timestamp_ms =
        Some(future.received_timestamp_ms + 3 * 24 * 60 * 60 * 1000);

    let rejected = recorder
        .db
        .with_txn(|txn| record_location(txn, &mut recorder.processor, &future, true))
        .unwrap();
    assert_eq!(rejected.process_result, ProcessResult::Ignore);
    assert!(!rejected.journey_finalized);
    assert_eq!(recorder.ongoing(), Some(range(&points[..1])));
    assert!(recorder
        .db
        .with_txn(|txn| txn.get_ongoing_journey_raw_data())
        .unwrap()
        .is_empty());

    let mut next = delivered(&points[1]);
    next.received_timestamp_ms -= 60_000;
    let accepted = recorder
        .db
        .with_txn(|txn| record_location(txn, &mut recorder.processor, &next, false))
        .unwrap();
    assert_ne!(accepted.process_result, ProcessResult::Ignore);
    assert_eq!(recorder.ongoing(), Some(range(&points[..2])));
}

#[test]
fn crossing_midnight_uses_the_location_date() {
    let mut recorder = Recorder::new();
    let first = point(23, 59, 0.0);
    let mut next = point(0, 0, 0.0001);
    next.timestamp_ms = Some(first.timestamp_ms.unwrap() + 60_000);
    recorder.record(&first);
    assert!(!recorder.record(&next).journey_finalized);
    let mut later = next.clone();
    later.timestamp_ms = Some(next.timestamp_ms.unwrap() + 30 * 60_000);
    later.point.longitude = 0.01;
    assert!(recorder.record(&later).journey_finalized);
    assert_eq!(recorder.ranges(), vec![range(&[first, next])]);
}
