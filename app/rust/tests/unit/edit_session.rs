use super::*;
use crate::journey_vector::TrackPoint;

fn segment(points: &[(f64, f64)]) -> TrackSegment {
    TrackSegment {
        track_points: points
            .iter()
            .map(|&(latitude, longitude)| TrackPoint {
                latitude,
                longitude,
            })
            .collect(),
    }
}

fn session(track_segments: Vec<TrackSegment>) -> EditSession {
    let data = JourneyVector { track_segments };
    let bitmap = EditSession::build_bitmap_from_vector(&data);
    EditSession {
        journey_id: String::new(),
        journey_revision: String::new(),
        map_renderer: Arc::new(Mutex::new(MapRenderer::new(bitmap))),
        initial_bounds: None,
        data,
        undo_stack: Vec::new(),
    }
}

#[test]
fn box_deletion_preserves_disjoint_segments_including_single_points() {
    let segments = vec![
        segment(&[(-1.0, -1.0)]),
        segment(&[(3.0, 3.0), (4.0, 4.0)]),
        segment(&[(1.0, 3.0)]),
    ];
    assert_eq!(
        EditSession::delete_points_in_box_segments(&segments, 0.0, 2.0, 0.0, 2.0),
        segments
    );
}

#[test]
fn box_deletion_removes_empty_and_contained_segments() {
    assert!(EditSession::delete_points_in_box_segments(&[], 0.0, 2.0, 0.0, 2.0).is_empty());
    for points in [
        vec![],
        vec![(1.0, 1.0)],
        vec![(0.0, 1.0)],
        vec![(2.0, 2.0)],
        vec![(0.5, 0.5), (1.5, 1.5)],
    ] {
        assert!(EditSession::delete_points_in_box_segments(
            &[segment(&points)],
            0.0,
            2.0,
            0.0,
            2.0
        )
        .is_empty());
    }
}

#[test]
fn box_deletion_splits_crossings_at_ordered_intersections() {
    for (start, entry, exit, end) in [
        ((1.0, -1.0), (1.0, 0.0), (1.0, 2.0), (1.0, 3.0)),
        ((-1.0, 1.0), (0.0, 1.0), (2.0, 1.0), (3.0, 1.0)),
        // Corner intersections must be deduplicated across the two edges.
        ((-1.0, -1.0), (0.0, 0.0), (2.0, 2.0), (3.0, 3.0)),
    ] {
        for (start, entry, exit, end) in [(start, entry, exit, end), (end, exit, entry, start)] {
            assert_eq!(
                EditSession::delete_points_in_box_segments(
                    &[segment(&[start, end])],
                    0.0,
                    2.0,
                    0.0,
                    2.0,
                ),
                vec![segment(&[start, entry]), segment(&[exit, end])]
            );
        }
    }
}

#[test]
fn box_deletion_clips_entering_and_leaving_segments() {
    assert_eq!(
        EditSession::delete_points_in_box_segments(
            &[segment(&[(1.0, -1.0), (1.0, 1.0), (1.0, 3.0)])],
            0.0,
            2.0,
            0.0,
            2.0,
        ),
        vec![
            segment(&[(1.0, -1.0), (1.0, 0.0)]),
            segment(&[(1.0, 2.0), (1.0, 3.0)]),
        ]
    );
}

#[test]
fn reversed_box_corners_and_undo_restore_single_points() {
    let original = vec![segment(&[(0.001, 0.001)]), segment(&[(0.003, 0.003)])];
    let mut session = session(original.clone());
    // A disjoint selection must not add an undo checkpoint.
    session
        .delete_points_in_box(0.01, 0.01, 0.02, 0.02)
        .unwrap();
    assert_eq!(session.data.track_segments, original);
    assert!(!session.can_undo());

    session
        .delete_points_in_box(0.002, 0.002, 0.0, 0.0)
        .unwrap();
    assert_eq!(session.data.track_segments, vec![original[1].clone()]);
    assert!(session.can_undo());
    session.undo().unwrap();
    assert_eq!(session.data.track_segments, original);
    assert!(!session.can_undo());
}

#[test]
fn drawing_deduplicates_adjacent_points_and_supports_undo() {
    let mut session = session(vec![]);
    assert!(matches!(
        session.add_lines(&[], false).unwrap(),
        AddLinesOutcome::Ignored
    ));
    assert!(matches!(
        session.add_lines(&[(0.0, 0.0), (0.0, 0.0)], false).unwrap(),
        AddLinesOutcome::Ignored
    ));
    assert!(!session.can_undo());
    assert!(matches!(
        session
            .add_lines(&[(0.0, 0.0), (0.0, 0.0), (0.001, 0.001)], false)
            .unwrap(),
        AddLinesOutcome::Added
    ));
    assert_eq!(
        session.data.track_segments,
        vec![segment(&[(0.0, 0.0), (0.001, 0.001)])]
    );
    session.undo().unwrap();
    assert!(session.data.track_segments.is_empty());
    assert!(!session.can_undo());
}

#[test]
fn linked_drawing_snaps_to_different_track_endpoints() {
    let mut session = session(vec![
        segment(&[(0.0, 0.0), (0.0, 0.001)]),
        segment(&[(0.0, 0.003), (0.0, 0.004)]),
    ]);
    assert!(matches!(
        session
            .add_lines(&[(0.0, 0.0011), (0.0, 0.0029)], true)
            .unwrap(),
        AddLinesOutcome::Added
    ));
    assert_eq!(
        session.data.track_segments[2],
        segment(&[(0.0, 0.001), (0.0, 0.003)])
    );
    session.undo().unwrap();
    let original = session.data.clone();
    assert!(matches!(
        session
            .add_lines(&[(0.0, 0.0), (0.0, 0.001)], true)
            .unwrap(),
        AddLinesOutcome::LinkedDrawInvalidLinkTargets
    ));
    assert!(matches!(
        session
            .add_lines(&[(1.0, 1.0), (1.0, 1.001)], true)
            .unwrap(),
        AddLinesOutcome::LinkedDrawTooFar
    ));
    assert_eq!(session.data, original);
    assert!(!session.can_undo());
}

use crate::flight_track_processor;
use crate::gps::{Point, RawGPSPoint};

#[test]
fn deleting_a_remote_box_preserves_dateline_flights() {
    for longitudes in [[179.99, -179.99], [-179.999, 179.999]] {
        let source = longitudes
            .map(|longitude| RawGPSPoint {
                point: Point {
                    latitude: 0.0,
                    longitude,
                },
                timestamp_ms: None,
                accuracy: None,
                altitude: None,
                speed: None,
            })
            .to_vec();
        let flight = flight_track_processor::process(&[source]).unwrap();
        let edited = EditSession::delete_points_in_box_segments(
            &flight.track_segments,
            -1.0,
            1.0,
            -1.0,
            1.0,
        );
        assert_eq!(edited, flight.track_segments);
    }
}
