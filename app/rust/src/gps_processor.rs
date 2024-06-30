use crate::{
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::OngoingJourney,
};
use anyhow::Result;
use chrono::DateTime;
#[derive(Clone, Debug, PartialEq)]
pub struct RawData {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp_ms: Option<i64>,
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
            _ => panic!("invalid `ProcessResult`"),
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
    pub fn preprocess<F>(&mut self, curr_data: RawData, f: F)
    where
        F: FnOnce(&Option<RawData>, &RawData, ProcessResult),
    {
        // TODO: the current implementation is still pretty naive.
        // Things we could do:
        // 1. tune the threshold, maybe use different values with different
        //    devices/speed. Maybe maintain a state about how the user is moving.
        // 2. ignore data that is too similar to the previous one or something
        //    like that.

        // TODO: we need distance filter
        const TIME_THRESHOLD_IN_MS: i64 = 5 * 1000;
        const ACCURACY_THRESHOLD: f32 = 12.0;
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
                    match curr_data
                        .timestamp_ms
                        .and_then(|now| last_data.timestamp_ms.map(|prev| now - prev))
                    {
                        None => ProcessResult::Append,
                        Some(time_diff_in_ms) => {
                            if time_diff_in_ms < 0 {
                                // NOTE: We could get a location update from a while ago and
                                // it can mess up the track. So we simply just drop these. If
                                // this turns out to be not good enough, our options are:
                                // 1. having a small buffer for updates, and sort events in
                                // the buffer; or
                                // 2. store these out of order events else where and add them
                                // back in `finalize_journey`.
                                ProcessResult::Ignore
                            } else if time_diff_in_ms > TIME_THRESHOLD_IN_MS {
                                ProcessResult::NewSegment
                            } else {
                                ProcessResult::Append
                            }
                        }
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

pub struct PreprocessedData {
    pub timestamp_sec: Option<i64>,
    pub track_point: TrackPoint,
    pub process_result: ProcessResult,
}

pub fn build_vector_journey(
    results: impl Iterator<Item = Result<PreprocessedData>>,
) -> Result<Option<OngoingJourney>> {
    let mut segmants = Vec::new();
    let mut current_segment = Vec::new();

    let mut start_timestamp_sec = None;
    let mut end_timestamp_sec = None;
    for result in results {
        let data = result?;
        if data.timestamp_sec.is_some() {
            end_timestamp_sec = data.timestamp_sec;
        }
        if start_timestamp_sec.is_none() {
            start_timestamp_sec = data.timestamp_sec;
        }
        let need_break = data.process_result == ProcessResult::NewSegment;
        if need_break && !current_segment.is_empty() {
            segmants.push(TrackSegment {
                track_points: current_segment,
            });
            current_segment = Vec::new();
        }
        if data.process_result != ProcessResult::Ignore {
            current_segment.push(data.track_point);
        }
    }
    if !current_segment.is_empty() {
        segmants.push(TrackSegment {
            track_points: current_segment,
        });
    }

    if segmants.is_empty() {
        Ok(None)
    } else {
        let start = start_timestamp_sec.map(|x| DateTime::from_timestamp(x, 0).unwrap());
        let end = end_timestamp_sec.map(|x| DateTime::from_timestamp(x, 0).unwrap());
        Ok(Some(OngoingJourney {
            start,
            end,
            journey_vector: JourneyVector {
                track_segments: segmants,
            },
        }))
    }
}
