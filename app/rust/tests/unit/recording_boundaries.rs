use super::*;
use crate::gps::RawGPSPoint;
use crate::main_db::MainDb;
use chrono::{TimeZone, Utc};
use tempdir::TempDir;

const NOW_MS: i64 = 1_790_000_000_000;

fn delivered(location: &RawGPSPoint) -> ExtendedRawGPSPoint {
    ExtendedRawGPSPoint {
        raw_gps_point: location.clone(),
        received_timestamp_ms: location.timestamp_ms.unwrap_or_default(),
    }
}

fn record(
    txn: &mut Txn,
    processor: &mut GpsPreprocessor,
    data: &ExtendedRawGPSPoint,
    raw: bool,
) -> Result<RecordedLocation> {
    Ok(
        record_locations(txn, processor, std::slice::from_ref(data), raw)?
            .pop()
            .unwrap(),
    )
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
        self.record_at(location, NOW_MS)
    }

    fn record_at(&mut self, location: &RawGPSPoint, received_at: i64) -> RecordedLocation {
        let mut next = self.processor.clone();
        let mut data = delivered(location);
        data.received_timestamp_ms = received_at;
        let result = self
            .db
            .with_txn(|txn| record(txn, &mut next, &data, false))
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
fn restart_preserves_the_boundary_and_raw_data() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    let updates: Vec<_> = points.iter().map(delivered).collect();
    recorder
        .db
        .with_txn(|txn| record_locations(txn, &mut recorder.processor, &updates[..3], true))
        .unwrap();
    recorder.restart();
    recorder
        .db
        .with_txn(|txn| {
            let recorded = record_locations(txn, &mut recorder.processor, &updates[3..], true)?;
            assert!(recorded[0].journey_finalized);
            assert_eq!(recorded[0].process_result, ProcessResult::NewSegment);
            assert!(recorded[0].previous_point.is_none());
            let saved = txn.query_journeys(None, None, None)?;
            assert_eq!(saved.len(), 1);
            let raw = txn
                .get_journey_raw_data(&saved[0].id)?
                .unwrap()
                .deserialize()?;
            assert_eq!(raw.points, updates[..3]);
            assert_eq!(txn.get_ongoing_journey_raw_data()?.points, updates[3..]);
            Ok(())
        })
        .unwrap();
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
    assert_eq!(recorder.ongoing(), Some(range(&points[3..])));
}

#[test]
fn invalid_fixes_do_not_split_or_advance_the_cursor() {
    let mut recorder = Recorder::new();
    let points = two_journeys();
    for point in &points[..3] {
        recorder.record(point);
    }
    let mut inaccurate = delivered(&points[3]);
    inaccurate.raw_gps_point.accuracy = Some(100.0);
    let mut bad_time = delivered(&points[3]);
    bad_time.raw_gps_point.timestamp_ms = Some(i64::MAX);
    let mut bad_coordinate = delivered(&points[3]);
    bad_coordinate.raw_gps_point.point.latitude = f64::NAN;
    let mut missing_time = delivered(&points[3]);
    missing_time.raw_gps_point.timestamp_ms = None;
    let mut future = delivered(&points[3]);
    future.received_timestamp_ms -= 1;
    recorder
        .db
        .with_txn(|txn| {
            for bad in [
                inaccurate.clone(),
                bad_time,
                bad_coordinate,
                missing_time,
                future,
            ] {
                let result = record(txn, &mut recorder.processor, &bad, true)?;
                assert!(!result.journey_finalized);
                assert_eq!(result.process_result, ProcessResult::Ignore);
            }
            assert_eq!(txn.get_ongoing_journey_raw_data()?.points, vec![inaccurate]);
            Ok(())
        })
        .unwrap();
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
    let updates: Vec<_> = points[3..].iter().map(delivered).collect();
    let mut next = recorder.processor.clone();
    let result: Result<()> = recorder.db.with_txn(|txn| {
        assert!(record_locations(txn, &mut next, &updates, false)?[0].journey_finalized);
        anyhow::bail!("injected transaction failure")
    });
    assert!(result.is_err());
    assert!(recorder.ranges().is_empty());
    assert_eq!(recorder.ongoing(), Some(range(&points[..3])));
    let recorded = recorder
        .db
        .with_txn(|txn| record_locations(txn, &mut recorder.processor, &updates, false))
        .unwrap();
    assert!(recorded[0].journey_finalized);
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
    assert_eq!(recorder.ongoing(), Some(range(&points[3..])));
}

#[test]
fn untimed_legacy_points_do_not_block_splitting_after_upgrade() {
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
    for point in &points[..3] {
        recorder.db.record(point, ProcessResult::Append).unwrap();
    }
    recorder.db.record(&untimed, ProcessResult::Append).unwrap();
    recorder.restart();

    let mut delayed = points[1].clone();
    delayed.timestamp_ms = delayed.timestamp_ms.map(|time| time + 500);
    assert_eq!(
        recorder.record(&delayed).process_result,
        ProcessResult::Ignore
    );
    assert!(recorder.record(&points[3]).journey_finalized);
    assert_eq!(recorder.ranges(), vec![range(&points[..3])]);
    assert_eq!(recorder.ongoing(), Some(range(&points[3..4])));
}

#[test]
fn finalization_clears_the_cursor_for_the_next_recording() {
    let mut recorder = Recorder::new();
    let before = point(10, 0, 0.0);
    recorder.record(&before);
    assert!(recorder
        .db
        .with_txn(|txn| txn.finalize_ongoing_journey(false))
        .unwrap());
    recorder.restart();

    let db = rusqlite::Connection::open(recorder.dir.path().join("main.db")).unwrap();
    let has_cursor: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM setting WHERE key = 'RECORDING_LAST_TIMESTAMP_MS')",
            (),
            |row| row.get(0),
        )
        .unwrap();
    assert!(!has_cursor);

    // Older builds could leave this value behind after finalizing.
    recorder
        .db
        .with_txn(|txn| txn.set_last_recording_timestamp_ms(before.timestamp_ms.unwrap()))
        .unwrap();
    let delayed = point(8, 0, 0.01);
    assert_eq!(
        recorder.record(&delayed).process_result,
        ProcessResult::NewSegment
    );
    assert_eq!(recorder.ongoing(), Some(range(&[delayed])));
    assert_eq!(recorder.ranges(), vec![range(&[before])]);
}

#[test]
fn clock_rollback_accepts_delayed_fixes_and_retries_failed_batches() {
    let mut before = point(10, 0, 0.0);
    let before_ms = before.timestamp_ms.unwrap() + 500;
    before.timestamp_ms = Some(before_ms);
    let mut caught_up = before.clone();
    caught_up.point.longitude = 0.02;

    for (rollback_ms, delivery_delay_ms, already_finalized) in [
        (20_000, 0, false),
        (7_200_000, 120_000, false),
        (7_200_000, 0, true),
    ] {
        let mut corrected = point(8, 0, 0.01);
        corrected.timestamp_ms = Some(before_ms - rollback_ms);
        let received_at = corrected.timestamp_ms.unwrap() + delivery_delay_ms;
        let mut data = delivered(&corrected);
        data.received_timestamp_ms = received_at;
        let updates = [data, delivered(&caught_up)];

        let mut recorder = Recorder::new();
        recorder.record_at(&before, before_ms);
        if already_finalized {
            assert!(recorder
                .db
                .with_txn(|txn| txn.finalize_ongoing_journey(false))
                .unwrap());
            recorder.restart();
        }

        // A fix captured before the rewind is now ahead of receipt time.
        assert_eq!(
            recorder.record_at(&before, received_at).process_result,
            ProcessResult::Ignore
        );

        let mut next = recorder.processor.clone();
        let result: Result<()> = recorder.db.with_txn(|txn| {
            let points = record_locations(txn, &mut next, &updates, false)?;
            let first = &points[0];
            assert_eq!(
                first.process_result,
                ProcessResult::NewSegment,
                "rollback {rollback_ms} ms, delivery delay {delivery_delay_ms} ms"
            );
            assert_eq!(first.journey_finalized, !already_finalized);
            anyhow::bail!("injected transaction failure")
        });
        assert!(result.is_err());
        assert_eq!(
            recorder.ongoing(),
            (!already_finalized).then(|| range(std::slice::from_ref(&before)))
        );

        let previous = (!already_finalized).then_some(before_ms);
        assert_eq!(
            recorder
                .db
                .with_txn(|txn| txn.last_recording_timestamp_ms())
                .unwrap(),
            previous
        );
        let retried = recorder
            .db
            .with_txn(|txn| record_locations(txn, &mut recorder.processor, &updates, false))
            .unwrap();
        assert_eq!(retried[0].journey_finalized, !already_finalized);
        assert_eq!(retried[0].process_result, ProcessResult::NewSegment);
        assert!(retried[0].previous_point.is_none());
        assert_eq!(
            recorder.ranges(),
            vec![range(std::slice::from_ref(&before))]
        );
        assert_eq!(
            recorder.ongoing(),
            Some(range(&[corrected, caught_up.clone()]))
        );
        recorder.restart();
        assert_eq!(
            recorder
                .db
                .with_txn(|txn| txn.last_recording_timestamp_ms())
                .unwrap(),
            Some(before_ms)
        );
    }
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
