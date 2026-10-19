use super::EditSession;
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
