use super::*;
use crate::gps::RawGPSPoint;
use crate::main_db::MainDb;
use chrono::Utc;
use rusqlite::Connection;
use tempdir::TempDir;

const BASE: i64 = 1_700_000_000_000;

struct Recorder {
    dir: TempDir,
    db: MainDb,
    preprocessor: GpsPreprocessor,
    raw: bool,
}

impl Recorder {
    fn new(raw: bool) -> Self {
        let dir = TempDir::new("recording-watermark").unwrap();
        let db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
        Self {
            dir,
            db,
            preprocessor: GpsPreprocessor::new(),
            raw,
        }
    }

    fn record(&mut self, point: &ExtendedRawGPSPoint) -> Result<RecordedLocation> {
        let mut next = self.preprocessor.clone();
        let recorded = self
            .db
            .with_txn(|txn| record_location(txn, &mut next, point, self.raw))?;
        self.preprocessor = next;
        Ok(recorded)
    }

    fn connection(&self) -> Connection {
        Connection::open(self.dir.path().join("main.db")).unwrap()
    }

    fn counts(&self) -> (i64, i64) {
        self.connection()
            .query_row(
                "SELECT (SELECT COUNT(*) FROM ongoing_journey),
                        (SELECT COUNT(*) FROM ongoing_journey_raw_data)",
                (),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    fn cursor(&mut self) -> Option<i64> {
        self.db
            .with_txn(|txn| txn.last_recording_timestamp_ms())
            .unwrap()
    }

    fn restart(&mut self) {
        self.db = MainDb::open(self.dir.path().to_str().unwrap()).unwrap();
        self.preprocessor = GpsPreprocessor::new();
    }
}

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

fn assert_same_preprocessor(before: &GpsPreprocessor, after: &GpsPreprocessor) {
    let mut before = before.clone();
    let mut after = after.clone();
    assert_eq!(before.last_kept_point(), after.last_kept_point());
    for probe in [point(31, 0.00003), point(32, 0.00004), point(45, 0.00005)] {
        assert_eq!(
            before.preprocess(&probe.raw_gps_point),
            after.preprocess(&probe.raw_gps_point)
        );
        assert_eq!(before.last_kept_point(), after.last_kept_point());
    }
}

#[test]
fn delayed_fixes_newer_than_the_persistent_cursor_keep_their_capture_time() {
    let mut recorder = Recorder::new(true);
    let first = point(1, 0.0);
    let delayed = point(20, 0.00002);
    assert!(
        delayed.received_timestamp_ms - delayed.raw_gps_point.timestamp_ms.unwrap() > 86_400_000
    );
    assert_eq!(
        recorder.record(&first).unwrap().process_result,
        ProcessResult::NewSegment
    );
    assert_eq!(
        recorder.record(&delayed).unwrap().process_result,
        ProcessResult::Append
    );
    assert_eq!(recorder.cursor(), delayed.raw_gps_point.timestamp_ms);
    let raw = recorder
        .db
        .with_txn(|txn| txn.get_ongoing_journey_raw_data())
        .unwrap();
    assert_eq!(raw.points, [first, delayed]);
}

#[test]
fn duplicate_and_late_fixes_do_not_reach_preprocessing_or_raw_data() {
    let mut recorder = Recorder::new(true);
    recorder.record(&point(1, 0.0)).unwrap();
    recorder.record(&point(20, 0.00002)).unwrap();
    let before = recorder.preprocessor.clone();
    for rejected in [point(20, 0.1), point(10, 0.2)] {
        let result = recorder.record(&rejected).unwrap();
        assert_eq!(result.process_result, ProcessResult::Ignore);
        assert!(result.previous_point.is_none());
    }
    assert_eq!(recorder.counts(), (2, 2));
    assert_eq!(recorder.cursor(), Some(BASE + 20_000));
    assert_same_preprocessor(&before, &recorder.preprocessor);
}

#[test]
fn timestamp_gate_rejects_the_one_twenty_ten_sequence_the_detector_accepts() {
    let points = [point(1, 0.0), point(20, 0.00002), point(10, 0.00004)];
    let mut old_preprocessor = GpsPreprocessor::new();
    for fix in &points[..2] {
        old_preprocessor.preprocess(&fix.raw_gps_point);
    }
    assert_eq!(
        old_preprocessor.preprocess(&points[2].raw_gps_point),
        ProcessResult::Append
    );

    let mut recorder = Recorder::new(true);
    recorder.record(&points[0]).unwrap();
    recorder.record(&points[1]).unwrap();
    assert_eq!(
        recorder.record(&points[2]).unwrap().process_result,
        ProcessResult::Ignore
    );
    assert_eq!(recorder.counts(), (2, 2));
    assert_eq!(recorder.cursor(), Some(BASE + 20_000));
}

#[test]
fn ignored_stationary_fixes_advance_the_millisecond_cursor_with_or_without_raw_data() {
    for raw in [false, true] {
        let mut recorder = Recorder::new(raw);
        recorder.record(&point(1, 0.0)).unwrap();
        let mut stationary = point(20, 0.0);
        stationary.raw_gps_point.timestamp_ms = Some(BASE + 20_123);
        assert_eq!(
            recorder.record(&stationary).unwrap().process_result,
            ProcessResult::Ignore
        );
        assert_eq!(recorder.cursor(), Some(BASE + 20_123));
        assert_eq!(recorder.counts(), (1, if raw { 2 } else { 0 }));
        recorder.restart();
        let mut older = point(20, 0.00002);
        older.raw_gps_point.timestamp_ms = Some(BASE + 20_122);
        assert_eq!(
            recorder.record(&older).unwrap().process_result,
            ProcessResult::Ignore
        );
        assert!(recorder.preprocessor.last_kept_point().is_none());
        assert_eq!(recorder.counts(), (1, if raw { 2 } else { 0 }));
    }
}

#[test]
fn replay_after_finalization_and_restart_cannot_recreate_a_journey() {
    let mut recorder = Recorder::new(true);
    let points = [point(1, 0.0), point(20, 0.00002)];
    recorder.record(&points[0]).unwrap();
    recorder.record(&points[1]).unwrap();
    assert!(recorder
        .db
        .with_txn(|txn| txn.finalize_ongoing_journey(true))
        .unwrap());
    recorder.restart();
    for fix in &points {
        assert_eq!(
            recorder.record(fix).unwrap().process_result,
            ProcessResult::Ignore
        );
    }
    assert_eq!(recorder.counts(), (0, 0));
    assert_eq!(recorder.cursor(), Some(BASE + 20_000));
    assert!(recorder.preprocessor.last_kept_point().is_none());
    assert_eq!(
        recorder
            .db
            .with_txn(|txn| txn.query_journeys(None, None, None))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        recorder.record(&point(21, 0.00003)).unwrap().process_result,
        ProcessResult::NewSegment
    );
}

#[test]
fn retried_poor_quality_fixes_may_repeat_diagnostics_without_advancing_recording() {
    for raw in [false, true] {
        let mut recorder = Recorder::new(raw);
        recorder.record(&point(1, 0.0)).unwrap();
        let before = recorder.preprocessor.clone();
        let mut inaccurate = point(20, 0.00002);
        inaccurate.raw_gps_point.accuracy = Some(100.0);
        for attempt in 1..=2 {
            assert_eq!(
                recorder.record(&inaccurate).unwrap().process_result,
                ProcessResult::Ignore
            );
            assert_eq!(recorder.cursor(), Some(BASE + 1000));
            assert_eq!(recorder.counts(), (1, if raw { 1 + attempt } else { 0 }));
            assert_same_preprocessor(&before, &recorder.preprocessor);
        }
        assert_eq!(
            recorder.record(&point(10, 0.00001)).unwrap().process_result,
            ProcessResult::Append
        );
        assert_eq!(recorder.cursor(), Some(BASE + 10_000));
        assert_eq!(recorder.counts(), (2, if raw { 4 } else { 0 }));
        if raw {
            let raw = recorder
                .db
                .with_txn(|txn| txn.get_ongoing_journey_raw_data())
                .unwrap();
            assert_eq!(raw.points[1], inaccurate);
            assert_eq!(raw.points[2], inaccurate);
        }
    }
}

#[test]
fn failed_transaction_rolls_back_points_raw_cursor_and_preprocessing() {
    for raw in [false, true] {
        let mut recorder = Recorder::new(raw);
        recorder.record(&point(1, 0.0)).unwrap();
        let before = recorder.preprocessor.clone();
        let connection = recorder.connection();
        connection
            .execute_batch(
                "CREATE TRIGGER fail_recording_cursor BEFORE INSERT ON setting
             WHEN NEW.key = 'RECORDING_LAST_TIMESTAMP_MS'
             BEGIN SELECT RAISE(ABORT, 'injected cursor failure'); END;",
            )
            .unwrap();
        let next = point(20, 0.00002);
        let error = recorder
            .record(&next)
            .err()
            .expect("cursor write must fail");
        assert!(format!("{error:#}").contains("injected cursor failure"));
        assert_eq!(recorder.counts(), (1, i64::from(raw)));
        assert_eq!(recorder.cursor(), Some(BASE + 1000));
        assert_same_preprocessor(&before, &recorder.preprocessor);
        connection
            .execute_batch("DROP TRIGGER fail_recording_cursor;")
            .unwrap();
        assert_eq!(
            recorder.record(&next).unwrap().process_result,
            ProcessResult::Append
        );
        assert_eq!(recorder.counts(), (2, if raw { 2 } else { 0 }));
        assert_eq!(recorder.cursor(), next.raw_gps_point.timestamp_ms);
    }
}

#[test]
fn malformed_times_coordinates_and_accuracy_do_not_change_recording_state() {
    let mut recorder = Recorder::new(true);
    recorder.record(&point(1, 0.0)).unwrap();
    let before = recorder.preprocessor.clone();
    let mut invalid = Vec::new();
    for timestamp in [None, Some(i64::MAX), Some(i64::MIN)] {
        let mut fix = point(20, 0.0);
        fix.raw_gps_point.timestamp_ms = timestamp;
        invalid.push(fix);
    }
    for (latitude, longitude) in [
        (f64::NAN, 0.0),
        (91.0, 0.0),
        (0.0, f64::INFINITY),
        (0.0, 181.0),
    ] {
        let mut fix = point(20, longitude);
        fix.raw_gps_point.point.latitude = latitude;
        invalid.push(fix);
    }
    for accuracy in [f32::NAN, f32::INFINITY, -1.0] {
        let mut fix = point(20, 0.0);
        fix.raw_gps_point.accuracy = Some(accuracy);
        invalid.push(fix);
    }
    for fix in &invalid {
        assert_eq!(
            recorder.record(fix).unwrap().process_result,
            ProcessResult::Ignore
        );
    }
    assert_eq!(recorder.counts(), (1, 1));
    assert_eq!(recorder.cursor(), Some(BASE + 1000));
    assert_same_preprocessor(&before, &recorder.preprocessor);
    assert_eq!(
        recorder.record(&point(10, 0.00001)).unwrap().process_result,
        ProcessResult::Append
    );
}

#[test]
fn upgrade_uses_the_legacy_second_until_a_precise_cursor_is_saved() {
    let mut recorder = Recorder::new(true);
    let mut latest = point(20, 0.00002);
    latest.raw_gps_point.timestamp_ms = Some(BASE + 20_900);
    for fix in [point(1, 0.0), point(10, 0.00001), latest] {
        recorder
            .db
            .record(&fix.raw_gps_point, ProcessResult::Append)
            .unwrap();
    }
    recorder.restart();
    assert_eq!(recorder.cursor(), Some(BASE + 20_000));
    for fix in [point(15, 0.2), point(20, 0.2)] {
        assert_eq!(
            recorder.record(&fix).unwrap().process_result,
            ProcessResult::Ignore
        );
    }
    assert_eq!(recorder.counts(), (3, 0));
    assert!(recorder.preprocessor.last_kept_point().is_none());
    let mut next = point(20, 0.00003);
    next.raw_gps_point.timestamp_ms = Some(BASE + 20_001);
    assert_eq!(
        recorder.record(&next).unwrap().process_result,
        ProcessResult::NewSegment
    );
    assert_eq!(recorder.cursor(), Some(BASE + 20_001));
    recorder.restart();
    for fix in [point(20, 0.2), next] {
        assert_eq!(
            recorder.record(&fix).unwrap().process_result,
            ProcessResult::Ignore
        );
    }
    assert_eq!(recorder.counts(), (4, 1));
    let mut precise_next = point(20, 0.00004);
    precise_next.raw_gps_point.timestamp_ms = Some(BASE + 20_002);
    assert_eq!(
        recorder.record(&precise_next).unwrap().process_result,
        ProcessResult::NewSegment
    );
    assert_eq!(recorder.cursor(), Some(BASE + 20_002));
}
