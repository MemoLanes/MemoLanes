#[derive(Copy, Clone, Debug)]
pub struct RawData {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp_ms: i64,
    pub accuracy: f32,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Copy, Clone)]
#[repr(i8)]
pub enum ProcessResult {
    Append = 0,
    NewSegment = 1,
    // negative values are for ones that should not be stored in the
    // `ongoing_journey` table.
    Ignore = -1,
}

impl ProcessResult {
    pub fn to_int(&self) -> i8 {
        *self as i8
    }
}

#[cfg(test)]
mod tests {
    use crate::gps_processor::ProcessResult;

    #[test]
    fn to_int() {
        assert_eq!(ProcessResult::NewSegment.to_int(), 1);
        assert_eq!(ProcessResult::Ignore.to_int(), -1);
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
        ProcessResult::Append
    }
}
