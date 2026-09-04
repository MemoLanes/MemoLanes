//! Journey-attached raw data, stored as independently framed protobuf records.
//!
//! R0 layout:
//!
//! ```text
//! "R0"
//! Zstd frame containing:
//!   header byte length: unsigned varint32
//!   JourneyRawDataHeaderProto bytes
//!   point byte length: unsigned varint32
//!   ExtendedRawGPSPointProto bytes
//!   ... (points in recording order, until the decompressed stream ends)
//! ```
//!
//! The header is mandatory, including for an empty recording. Every protobuf
//! has its own length prefix; there is no enclosing protobuf or total point
//! count. Header and point fields can be extended independently. A future
//! implementation can process these records incrementally without changing
//! the format. The current API still returns all points in memory.
//!
//! EOF is valid only between point records. Incomplete lengths or payloads
//! are errors. The filesystem CSV format is isolated in `crate::legacy_raw_data`.

use std::io::Cursor;

use anyhow::Result;
use protobuf::{CodedInputStream, CodedOutputStream, Message};

use crate::{gps_processor::Point, journey_data, protos, utils};

const JOURNEY_RAW_DATA_MAGIC_HEADER: [u8; 2] = *b"R0";

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
pub struct JourneyRawDataHeader {
    /// UTC creation time of this raw-data attachment, in milliseconds.
    pub created_at_timestamp_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JourneyRawData {
    pub header: JourneyRawDataHeader,
    pub points: Vec<ExtendedRawGPSPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SerializedJourneyRawData {
    bytes: Vec<u8>,
}

impl ExtendedRawGPSPoint {
    pub(crate) fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.to_proto().write_to_bytes()?)
    }

    pub(crate) fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self::of_proto(
            protos::raw_data::ExtendedRawGPSPointProto::parse_from_bytes(bytes)?,
        ))
    }

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

impl JourneyRawDataHeader {
    fn to_proto(&self) -> protos::raw_data::JourneyRawDataHeaderProto {
        let mut proto = protos::raw_data::JourneyRawDataHeaderProto::new();
        proto.created_at_timestamp_ms = self.created_at_timestamp_ms;
        proto
    }

    fn of_proto(proto: protos::raw_data::JourneyRawDataHeaderProto) -> Self {
        Self {
            created_at_timestamp_ms: proto.created_at_timestamp_ms,
        }
    }
}

impl JourneyRawData {
    pub fn new(points: Vec<ExtendedRawGPSPoint>, created_at_timestamp_ms: i64) -> Self {
        Self {
            header: JourneyRawDataHeader {
                created_at_timestamp_ms,
            },
            points,
        }
    }

    pub fn serialize(&self) -> Result<SerializedJourneyRawData> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&JOURNEY_RAW_DATA_MAGIC_HEADER);

        let mut encoder = zstd::Encoder::new(&mut bytes, journey_data::ZSTD_COMPRESS_LEVEL)?;
        {
            let mut output = CodedOutputStream::new(&mut encoder);
            self.header
                .to_proto()
                .write_length_delimited_to(&mut output)?;
            for point in &self.points {
                point.to_proto().write_length_delimited_to(&mut output)?;
            }
            output.flush()?;
        }
        encoder.finish()?;

        Ok(SerializedJourneyRawData { bytes })
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

impl SerializedJourneyRawData {
    pub fn deserialize(&self) -> Result<JourneyRawData> {
        let mut reader = Cursor::new(self.as_bytes());
        utils::validate_magic_header(&mut reader, &JOURNEY_RAW_DATA_MAGIC_HEADER)?;

        let mut decoder = zstd::Decoder::new(reader)?;
        let mut input = CodedInputStream::new(&mut decoder);
        let header = JourneyRawDataHeader::of_proto(
            protos::raw_data::JourneyRawDataHeaderProto::parse_from_bytes(&input.read_bytes()?)?,
        );
        let mut points = Vec::new();
        while !input.eof()? {
            points.push(ExtendedRawGPSPoint::deserialize(&input.read_bytes()?)?);
        }
        Ok(JourneyRawData { header, points })
    }

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
