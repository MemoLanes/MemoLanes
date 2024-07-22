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

impl RawData {
    pub fn haversine_distance(&self, other: &RawData) -> f64 {
        use std::f64::consts::PI;
        let r = 6371e3; // Earth's radius in meters

        let phi1 = self.latitude * PI / 180.0;
        let phi2 = other.latitude * PI / 180.0;
        let delta_phi = (other.latitude - self.latitude) * PI / 180.0;
        let delta_lambda = (other.longitude - self.longitude) * PI / 180.0;

        let a = (delta_phi / 2.0).sin().powi(2)
            + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        r * c // Distance in meters
    }
}

#[cfg(test)]
mod raw_data_tests {
    fn raw_data(latitude: f64, longitude: f64) -> super::RawData {
        super::RawData {
            latitude,
            longitude,
            timestamp_ms: None,
            accuracy: None,
            altitude: None,
            speed: None,
        }
    }

    #[test]
    fn haversine_distance() {
        let raw_data1 = raw_data(22.291608437, 114.202901212);
        let raw_data2 = raw_data(22.2914913837, 114.2018426615);

        assert_eq!(raw_data1.haversine_distance(&raw_data1) as i32, 0);
        assert_eq!(raw_data1.haversine_distance(&raw_data2) as i32, 109);
        assert_eq!(raw_data2.haversine_distance(&raw_data1) as i32, 109);

        // antimeridian
        assert_eq!(
            raw_data(0.0, -179.9).haversine_distance(&raw_data(0.0, 179.9)) as i32,
            22238
        );
        assert_eq!(
            raw_data(0.0, 179.9).haversine_distance(&raw_data(0.0, -179.9)) as i32,
            22238
        );
    }
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
mod process_result_tests {
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

    pub fn last_data(&self) -> Option<RawData> {
        self.last_data.clone()
    }

    pub fn preprocess(&mut self, curr_data: &RawData) -> ProcessResult {
        // TODO: the current implementation is still pretty naive.
        // Things we could do:
        // 1. Tune the threshold, maybe use different values with different
        //    devices/speed. Maybe maintain a state about how the user is moving.
        // 2. Ignore data that is too similar to the previous one or something
        //    like that. (Maybe having a moving and a stationary state,
        //    automatically switching between these).
        //
        // Something to note:
        // * Accuracy is not well defined. The unit is meters but: On android,
        //  it is the radius of this location at the 68th percentile confidence
        //  level. On iOS, it is not specified in document. It seems the
        //  accuaracy is always poor (higher in value) on iOS, maybe it is using
        //  95th percentile. I am not use, no one is normalizing this value, we
        // might need to use different threshold and tune it ourselves.
        //
        // * Values in GPX file are just from the device and we lose the device
        //   info (According to the bahvior of Guru Map). So we might need to
        //   use the iOS threshold or tune a new one. I am not sure. :(

        const TIME_THRESHOLD_IN_MS: i64 = 5 * 1000;
        const ACCURACY_THRESHOLD: f32 = 50.0;
        const SPEED_THRESHOLD: f64 = 250.0; // m/s
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
                    let mut time_diff_in_ms = curr_data
                        .timestamp_ms
                        .and_then(|now| last_data.timestamp_ms.map(|prev| now - prev));

                    let time_based_result = match time_diff_in_ms {
                        None => ProcessResult::Append,
                        Some(diff_in_ms) => {
                            if diff_in_ms < 0 {
                                // NOTE: We could get a location update from a while ago and
                                // it can mess up the track. So we simply just drop these. If
                                // this turns out to be not good enough, our options are:
                                // 1. having a small buffer for updates, and sort events in
                                // the buffer; or
                                // 2. store these out of order events else where and add them
                                // back in `finalize_journey`.
                                time_diff_in_ms = None;
                                ProcessResult::Ignore
                            } else if diff_in_ms > TIME_THRESHOLD_IN_MS {
                                ProcessResult::NewSegment
                            } else {
                                ProcessResult::Append
                            }
                        }
                    };

                    if time_based_result == ProcessResult::Append {
                        // let's consider (speed) distance now
                        let time_in_sec =
                            time_diff_in_ms.unwrap_or(TIME_THRESHOLD_IN_MS) as f64 / 1000.0;
                        let speed = curr_data.haversine_distance(last_data) / time_in_sec.max(0.01);
                        if speed < SPEED_THRESHOLD {
                            ProcessResult::Append
                        } else {
                            ProcessResult::NewSegment
                        }
                    } else {
                        time_based_result
                    }
                }
            }
        };
        if result != ProcessResult::Ignore {
            self.last_data = Some(curr_data.clone());
        };
        result
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
