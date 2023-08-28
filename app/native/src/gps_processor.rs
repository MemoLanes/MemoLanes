pub struct RawData {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp_ms: i64,
    pub accuracy: f32,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Copy, Clone)]
pub enum ProcessResult {
    Append = 0,
    Ignore = 1,
    NewSegment = 2,
    NewJourney = 3,
}

impl ProcessResult {
    pub fn to_int(&self) -> u8 {
        *self as u8
    }
}

pub struct GpsProcessor {}

impl GpsProcessor {
    pub fn new() -> Self {
        GpsProcessor {}
    }

    pub fn process(&self, _raw_data: &RawData) -> ProcessResult {
        // TODO: implement this. A naive version could be:
        // 1. Ingore data with low accuracy
        // 2. If distance/time change is big (maybe also consider speed), start a new segment.
        // 2. If distance/time change is way too big, start a new journey.
        ProcessResult::Append
    }
}
