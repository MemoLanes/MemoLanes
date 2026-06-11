use crate::gps_processor::Point;

#[derive(Clone, Debug, PartialEq)]
pub struct RawGPSPoint {
    pub point: Point,
    pub timestamp_ms: Option<i64>,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

pub struct ExtendedRawGPSPoint {
    pub raw_gps_point: RawGPSPoint,
    pub received_timestamp_ms: i64,
}

pub struct JourneyRawData {
    pub points: Vec<ExtendedRawGPSPoint>,
}

pub struct SerializedJourneyRawData {
    bytes: Vec<u8>,
}
