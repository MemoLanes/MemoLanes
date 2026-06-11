use crate::gps_processor::Point;

#[derive(Clone, Debug, PartialEq)]
pub struct RawDataPoint {
    pub point: Point,
    pub timestamp_ms: Option<i64>,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}
