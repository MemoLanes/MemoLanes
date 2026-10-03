use memolanes_core::{
    export_data,
    gps_processor::Point,
    raw_data::{
        ExtendedRawGPSPoint, JourneyRawData, JourneyRawDataHeader, RawGPSPoint,
        SerializedJourneyRawData,
    },
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
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
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

    let serialized = raw_data.serialize().unwrap();

    assert!(serialized.as_bytes().starts_with(b"R0"));
    assert_eq!(serialized.deserialize().unwrap(), raw_data);
}

#[test]
fn rejects_invalid_magic_header() {
    let serialized = SerializedJourneyRawData::from_bytes(vec![b'X', b'0']);

    let error = serialized.deserialize().unwrap_err().to_string();

    assert!(error.contains("Invalid magic header"));
}

fn framed_raw_data(payload: &[u8]) -> SerializedJourneyRawData {
    let mut bytes = b"R0".to_vec();
    bytes.extend(zstd::stream::encode_all(payload, 3).unwrap());
    SerializedJourneyRawData::from_bytes(bytes)
}

#[test]
fn raw_data_uses_a_header_followed_by_independently_framed_points() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 150,
        },
        points: vec![
            point(0., 0., None, None, None, None, 7),
            point(0., 0., None, None, None, None, 0),
        ],
    };
    // Header: field 1 = 150. First point: field 7 = 7.
    // The second point has all default values, so its protobuf is empty.
    let payload = [3, 0x08, 0x96, 0x01, 2, 0x38, 7, 0];
    let serialized = raw_data.serialize().unwrap();
    assert_eq!(
        zstd::stream::decode_all(&serialized.as_bytes()[2..]).unwrap(),
        payload
    );
    assert_eq!(framed_raw_data(&payload).deserialize().unwrap(), raw_data);
}

#[test]
fn raw_data_header_can_exist_without_points() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 150,
        },
        points: Vec::new(),
    };
    assert_eq!(
        raw_data.serialize().unwrap().deserialize().unwrap(),
        raw_data
    );
    assert_eq!(
        framed_raw_data(&[0])
            .deserialize()
            .unwrap()
            .header
            .created_at_timestamp_ms,
        0
    );
}

#[test]
fn raw_data_skips_future_header_and_point_fields() {
    // Unknown string field 2 in the header, and string field 8 in the point.
    let payload = [
        8, 0x08, 0x96, 0x01, 0x12, 3, b'a', b'b', b'c', 7, 0x38, 7, 0x42, 3, b'x', b'y', b'z',
    ];
    let raw_data = framed_raw_data(&payload).deserialize().unwrap();
    assert_eq!(raw_data.header.created_at_timestamp_ms, 150);
    assert_eq!(
        raw_data.points,
        vec![point(0., 0., None, None, None, None, 7)]
    );
}

#[test]
fn raw_data_rejects_missing_header_and_truncated_or_invalid_records() {
    for payload in [
        &[][..],                // missing header
        &[0x80],                // truncated header length
        &[3, 0x08],             // truncated header payload
        &[3, 0x08, 0x00],       // valid header protobuf, but shorter than declared
        &[0, 0x80],             // truncated point length
        &[0, 2, 0x38],          // truncated point payload
        &[0, 3, 0x38, 7],       // valid point protobuf, but shorter than declared
        &[0, 2, 0x38, 7, 0x80], // truncated length after a valid point
        &[0, 1, 0],             // invalid protobuf tag in a point
    ] {
        assert!(
            framed_raw_data(payload).deserialize().is_err(),
            "payload: {payload:?}"
        );
    }
}

#[test]
fn raw_data_rejects_truncated_compression_frame() {
    let mut bytes = JourneyRawData::new(Vec::new(), 1_700_000_000_000)
        .serialize()
        .unwrap()
        .into_bytes();
    bytes.pop();
    assert!(SerializedJourneyRawData::from_bytes(bytes)
        .deserialize()
        .is_err());
}

#[test]
fn journey_raw_data_exports_to_csv_gpx_and_kml() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
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

#[test]
fn journey_raw_data_gpx_uses_received_timestamp_as_fallback() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
        points: vec![point(
            31.230416,
            121.473701,
            None,
            None,
            None,
            None,
            1_700_000_001_234,
        )],
    };

    let mut gpx = Cursor::new(Vec::new());
    export_data::gpx::journey_raw_data_to_gpx_file(&raw_data, &mut gpx).unwrap();
    gpx.set_position(0);
    let gpx = gpx::read(gpx).unwrap();
    let exported_time: time::OffsetDateTime =
        gpx.tracks[0].segments[0].points[0].time.unwrap().into();

    assert_eq!(
        exported_time.unix_timestamp_nanos(),
        1_700_000_001_234_000_000
    );
}

#[test]
fn journey_raw_data_gpx_rejects_out_of_range_timestamp() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
        points: vec![point(
            31.230416,
            121.473701,
            Some(i64::MAX),
            None,
            None,
            None,
            1_700_000_001_234,
        )],
    };

    let error =
        export_data::gpx::journey_raw_data_to_gpx_file(&raw_data, &mut Cursor::new(Vec::new()))
            .unwrap_err();

    assert!(format!("{error:#}").contains("timestamp"));
}

#[test]
fn journey_raw_data_kml_orders_track_fields_and_rejects_invalid_timestamps() {
    let raw_data = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
        points: vec![
            point(
                31.230416,
                121.473701,
                None,
                None,
                None,
                None,
                1_700_000_001_234,
            ),
            point(
                31.230417,
                121.473702,
                None,
                None,
                None,
                None,
                1_700_000_002_234,
            ),
        ],
    };
    let mut output = Cursor::new(Vec::new());

    export_data::kml::journey_raw_data_to_kml_file(&raw_data, &mut output).unwrap();
    let kml = String::from_utf8(output.into_inner()).unwrap();
    let second_when = kml.rfind("<when>").unwrap();
    let first_coord = kml.find("<gx:coord>").unwrap();
    assert!(second_when < first_coord);

    let invalid = JourneyRawData {
        header: JourneyRawDataHeader {
            created_at_timestamp_ms: 1_700_000_000_000,
        },
        points: vec![point(
            31.230416,
            121.473701,
            Some(i64::MAX),
            None,
            None,
            None,
            1_700_000_001_234,
        )],
    };
    let error =
        export_data::kml::journey_raw_data_to_kml_file(&invalid, &mut Cursor::new(Vec::new()))
            .unwrap_err();
    assert!(format!("{error:#}").contains("timestamp"));
}
