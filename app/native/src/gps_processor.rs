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

pub struct GpsProcessor {
    last_data: Option<RawData>,

}

impl GpsProcessor {
    pub fn new() -> Self {
        GpsProcessor {
            last_data: None,
        }
    }

    pub fn process(&mut self, _raw_data: &RawData) -> ProcessResult {
        const  TIME_THRESHOLD: i64 = 30 * 1000;
        const  HORIZONTAL_ACCURACY_THRESHOLD: f32 = 50.0;
        let curr_data = _raw_data.clone();
        match self.last_data.take() {
            Some(last_data) => {
                let time_diff = curr_data.timestamp_ms - last_data.timestamp_ms;
                // Ignore the data if the precision is too small
                if curr_data.accuracy > HORIZONTAL_ACCURACY_THRESHOLD {
                    self.last_data = None;
                    return ProcessResult::NewSegment;
                } else if time_diff > TIME_THRESHOLD {
                    self.last_data = Some(curr_data);
                    return ProcessResult::Ignore;
                } else {
                    self.last_data = Some(curr_data);
                    return ProcessResult::Append;
                }
            }
            None => {
                // No last location information is directly considered trustworthy
                self.last_data = Some(curr_data);
                return ProcessResult::Append;
            }
        }
    }
}
