use memolanes_core::{
    export_data,
    gps_processor::Point,
    raw_data::{self, ExtendedRawGPSPoint, JourneyRawData, RawGPSPoint, SerializedJourneyRawData},
};
use std::io::Cursor;

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

#[test]
fn journey_raw_data_exports_to_csv_gpx_and_kml() {
    let raw_data = JourneyRawData {
        points: vec![point(
            31.230416,
            121.473701,
            Some(1_700_000_000_000),
            Some(4.5),
            Some(12.0),
            Some(1.25),
            1_700_000_000_010,
        )],
    };

    let mut csv = Vec::new();
    export_data::raw_csv::journey_raw_data_to_csv_file(&raw_data, &mut csv).unwrap();
    let csv = String::from_utf8(csv).unwrap();
    assert!(csv.contains("timestamp_ms,received_timestamp_ms,latitude,longitude"));
    assert!(csv.contains("31.230416,121.473701"));

    let mut gpx = Cursor::new(Vec::new());
    export_data::gpx::journey_raw_data_to_gpx_file(&raw_data, &mut gpx).unwrap();
    let gpx = String::from_utf8(gpx.into_inner()).unwrap();
    assert!(gpx.contains("MemoLanes RawData"));
    assert!(gpx.contains("lat=\"31.230416\" lon=\"121.473701\""));

    let mut kml = Cursor::new(Vec::new());
    export_data::kml::journey_raw_data_to_kml_file(&raw_data, &mut kml).unwrap();
    let kml = String::from_utf8(kml.into_inner()).unwrap();
    assert!(kml.contains("MemoLanes Raw Data"));
    assert!(kml.contains("2023-11-14T22:13:20.000Z"));
    assert!(kml.contains("121.473701 31.230416 12"));
}
