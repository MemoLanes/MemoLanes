use memolanes_core::{
    gps_processor::Point,
    raw_data::{self, ExtendedRawGPSPoint, JourneyRawData, RawGPSPoint, SerializedJourneyRawData},
};

fn point(
    latitude: f64,
    longitude: f64,
    timestamp_ms: Option<i64>,
    accuracy: Option<f32>,
    altitude: Option<f32>,
    speed: Option<f32>,
    received_timestamp_ms: i64,
) -> ExtendedRawGPSPoint {
    ExtendedRawGPSPoint {
        raw_gps_point: RawGPSPoint {
            point: Point {
                latitude,
                longitude,
            },
            timestamp_ms,
            accuracy,
            altitude,
            speed,
        },
        received_timestamp_ms,
    }
}

#[test]
fn journey_raw_data_round_trips_through_serialized_form() {
    let raw_data = JourneyRawData {
        points: vec![
            point(
                31.230416,
                121.473701,
                Some(1_700_000_000_000),
                Some(4.5),
                Some(12.0),
                Some(1.25),
                1_700_000_000_010,
            ),
            point(-33.86882, 151.20929, None, None, Some(42.0), None, 42),
        ],
    };

    let serialized = raw_data::serialize(&raw_data).unwrap();

    assert!(serialized.as_bytes().starts_with(b"R0"));
    assert_eq!(raw_data::deserialize(&serialized).unwrap(), raw_data);
}

#[test]
fn rejects_invalid_magic_header() {
    let serialized = SerializedJourneyRawData::from_bytes(vec![b'X', b'0']);

    let error = raw_data::deserialize(&serialized).unwrap_err().to_string();

    assert!(error.contains("Invalid magic header"));
}
