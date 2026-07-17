use std::io::Cursor;

use anyhow::Result;
use protobuf::Message;

use crate::{gps_processor::Point, journey_data, protos, utils};

const JOURNEY_RAW_DATA_MAGIC_HEADER: [u8; 2] = [b'R', b'0'];

#[derive(Clone, Debug, PartialEq)]
pub struct RawGPSPoint {
    pub point: Point,
    pub timestamp_ms: Option<i64>,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtendedRawGPSPoint {
    pub raw_gps_point: RawGPSPoint,
    pub received_timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JourneyRawData {
    pub points: Vec<ExtendedRawGPSPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerializedJourneyRawData {
    bytes: Vec<u8>,
}

impl ExtendedRawGPSPoint {
    fn to_proto(&self) -> protos::raw_data::ExtendedRawGPSPointProto {
        let ExtendedRawGPSPoint {
            raw_gps_point,
            received_timestamp_ms,
        } = self;
        let RawGPSPoint {
            point,
            timestamp_ms,
            accuracy,
            altitude,
            speed,
        } = raw_gps_point;
        let Point {
            latitude,
            longitude,
        } = point;

        let mut proto = protos::raw_data::ExtendedRawGPSPointProto::new();
        proto.timestamp_ms = *timestamp_ms;
        proto.latitude = *latitude;
        proto.longitude = *longitude;
        proto.accuracy = *accuracy;
        proto.altitude = *altitude;
        proto.speed = *speed;
        proto.received_timestamp_ms = *received_timestamp_ms;
        proto
    }

    fn of_proto(proto: protos::raw_data::ExtendedRawGPSPointProto) -> Self {
        ExtendedRawGPSPoint {
            raw_gps_point: RawGPSPoint {
                point: Point {
                    latitude: proto.latitude,
                    longitude: proto.longitude,
                },
                timestamp_ms: proto.timestamp_ms,
                accuracy: proto.accuracy,
                altitude: proto.altitude,
                speed: proto.speed,
            },
            received_timestamp_ms: proto.received_timestamp_ms,
        }
    }
}

impl JourneyRawData {
    fn to_proto(&self) -> protos::raw_data::JourneyRawDataProto {
        let JourneyRawData { points } = self;

        let mut proto = protos::raw_data::JourneyRawDataProto::new();
        proto.points = points.iter().map(ExtendedRawGPSPoint::to_proto).collect();
        proto
    }

    fn of_proto(proto: protos::raw_data::JourneyRawDataProto) -> Self {
        JourneyRawData {
            points: proto
                .points
                .into_iter()
                .map(ExtendedRawGPSPoint::of_proto)
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

impl SerializedJourneyRawData {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        SerializedJourneyRawData { bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl AsRef<[u8]> for SerializedJourneyRawData {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

pub(crate) fn serialize_point(point: &ExtendedRawGPSPoint) -> Result<Vec<u8>> {
    Ok(point.to_proto().write_to_bytes()?)
}

pub(crate) fn deserialize_point(bytes: &[u8]) -> Result<ExtendedRawGPSPoint> {
    Ok(ExtendedRawGPSPoint::of_proto(
        protos::raw_data::ExtendedRawGPSPointProto::parse_from_bytes(bytes)?,
    ))
}

pub fn serialize(raw_data: &JourneyRawData) -> Result<SerializedJourneyRawData> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&JOURNEY_RAW_DATA_MAGIC_HEADER);

    let mut encoder = zstd::Encoder::new(&mut bytes, journey_data::ZSTD_COMPRESS_LEVEL)?;
    raw_data.to_proto().write_to_writer(&mut encoder)?;
    encoder.finish()?;

    Ok(SerializedJourneyRawData { bytes })
}

pub fn deserialize(serialized: &SerializedJourneyRawData) -> Result<JourneyRawData> {
    let mut reader = Cursor::new(serialized.as_bytes());
    utils::validate_magic_header(&mut reader, &JOURNEY_RAW_DATA_MAGIC_HEADER)?;

    let mut decoder = zstd::Decoder::new(reader)?;
    let proto = protos::raw_data::JourneyRawDataProto::parse_from_reader(&mut decoder)?;
    Ok(JourneyRawData::of_proto(proto))
}
