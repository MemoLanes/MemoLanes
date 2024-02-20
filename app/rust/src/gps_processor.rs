use crate::{
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::OngoingJourney,
};
use anyhow::Result;
use chrono::DateTime;
#[derive(Clone, Debug)]
pub struct RawData {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp_ms: i64,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum ProcessResult {
    Append = 0,
    NewSegment = 1,
    // negative values are for ones that should not be stored in the
    // `ongoing_journey` table.
    Ignore = -1,
}

impl From<i8> for ProcessResult {
    fn from(i: i8) -> Self {
        match i {
            0 => ProcessResult::Append,
            1 => ProcessResult::NewSegment,
            -1 => ProcessResult::Ignore,
            _ => panic!("invalid ProcessResult"),
        }
    }
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
        GpsProcessor { last_data: None }
    }

    // the `f` here just a trick to avoid additional copy of `RawData`, one
    // could argue that this is over optimization (this does make the control
    // flow of this code a lot more complicated, so maybe we shouldn't do this).
    //  ¯\_(ツ)_/¯
    pub fn process<F>(&mut self, curr_data: RawData, f: F)
    where
        F: FnOnce(&Option<RawData>, &RawData, ProcessResult),
    {
        // TODO: the current implementation is still pretty naive.
        // Things we could do:
        // 1. tune the threshold, maybe use different values with different
        //    devices/speed. Maybe maintain a state about how the user is moving.
        // 2. ignore data that is too similar to the previous one or something
        //    like that.
        const TIME_THRESHOLD_IN_MS: i64 = 5 * 1000;
        const ACCURACY_THRESHOLD: f32 = 10.0;
        let should_ignore = match curr_data.accuracy {
            Some(accuracy) => accuracy > ACCURACY_THRESHOLD,
            None => false,
        };

        let result = if should_ignore {
            ProcessResult::Ignore
        } else {
            match &self.last_data {
                None => ProcessResult::NewSegment,
                Some(last_data) => {
                    let time_diff_in_ms = curr_data.timestamp_ms - last_data.timestamp_ms;
                    if time_diff_in_ms > TIME_THRESHOLD_IN_MS {
                        ProcessResult::NewSegment
                    } else {
                        ProcessResult::Append
                    }
                }
            }
        };
        f(&self.last_data, &curr_data, result);
        if result != ProcessResult::Ignore {
            self.last_data = Some(curr_data);
        }
    }
}

pub struct RawSegmentData {
    pub timestamp_sec: i64,
    pub track_point: TrackPoint,
    pub process_result: ProcessResult,
}

pub fn process_segment(
    results: impl Iterator<Item = Result<RawSegmentData>>,
) -> Result<Option<OngoingJourney>> {
    let mut segmants = Vec::new();
    let mut current_segment = Vec::new();

    let mut start_timestamp_sec = None;
    let mut end_timestamp_sec = None;
    for result in results {
        let data = result?;
        end_timestamp_sec = Some(data.timestamp_sec);
        if start_timestamp_sec.is_none() {
            start_timestamp_sec = Some(data.timestamp_sec);
        }
        let need_break = data.process_result.to_int() == ProcessResult::NewSegment.to_int();
        if need_break && !current_segment.is_empty() {
            segmants.push(TrackSegment {
                track_points: current_segment,
            });
            current_segment = Vec::new();
        }
        current_segment.push(data.track_point);
    }
    if !current_segment.is_empty() {
        segmants.push(TrackSegment {
            track_points: current_segment,
        });
    }

    if segmants.is_empty() {
        Ok(None)
    } else {
        // must be `Some`
        let start = DateTime::from_timestamp(start_timestamp_sec.unwrap(), 0).unwrap();
        let end = DateTime::from_timestamp(end_timestamp_sec.unwrap(), 0).unwrap();
        Ok(Some(OngoingJourney {
            start,
            end,
            journey_vector: JourneyVector {
                track_segments: segmants,
            },
        }))
    }
}
