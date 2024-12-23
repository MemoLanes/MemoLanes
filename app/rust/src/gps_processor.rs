use crate::{
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::OngoingJourney,
};
use anyhow::Result;
use chrono::DateTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}

impl Point {
    pub fn haversine_distance(&self, other: &Point) -> f64 {
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

#[derive(Clone, Debug, PartialEq)]
pub struct RawData {
    pub point: Point,
    pub timestamp_ms: Option<i64>,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[cfg(test)]
mod point_tests {
    fn point(latitude: f64, longitude: f64) -> super::Point {
        super::Point {
            latitude,
            longitude,
        }
    }

    #[test]
    fn haversine_distance() {
        let point1 = point(22.291608437, 114.202901212);
        let point2 = point(22.2914913837, 114.2018426615);

        assert_eq!(point1.haversine_distance(&point1) as i32, 0);
        assert_eq!(point1.haversine_distance(&point2) as i32, 109);
        assert_eq!(point2.haversine_distance(&point1) as i32, 109);

        // antimeridian
        assert_eq!(
            point(0.0, -179.9).haversine_distance(&point(0.0, 179.9)) as i32,
            22238
        );
        assert_eq!(
            point(0.0, 179.9).haversine_distance(&point(0.0, -179.9)) as i32,
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

enum GpsPreprocessorState {
    Empty,
    Moving {
        last_point: Point,
        last_timestamp_ms: Option<i64>,
        possible_center_point: Point,
        timestamp_ms_when_center_point_picked: Option<i64>,
        num_of_data_since_center_point_picked: i64,
    },
    Stationary {
        center_point: Point,
        last_timestamp_ms: Option<i64>,
    },
}

pub struct GpsPreprocessor {
    state: GpsPreprocessorState,
}

impl GpsPreprocessor {
    pub fn new() -> Self {
        GpsPreprocessor {
            state: GpsPreprocessorState::Empty,
        }
    }

    pub fn last_point(&self) -> Option<Point> {
        use GpsPreprocessorState::*;
        match &self.state {
            Empty => None,
            Moving { last_point, .. }
            | Stationary {
                center_point: last_point,
                ..
            } => Some(last_point.clone()),
        }
    }

    // 1. Accuracy is too bad.
    // 2. Timestamp moving backwards.
    fn is_bad_data(&self, curr_data: &RawData) -> bool {
        use GpsPreprocessorState::*;
        const ACCURACY_THRESHOLD: f32 = 50.0;
        if let Some(accuracy) = curr_data.accuracy {
            if accuracy > ACCURACY_THRESHOLD {
                return true;
            }
        }

        let last_timestamp_ms = match &self.state {
            Empty => None,
            Moving {
                last_timestamp_ms, ..
            }
            | Stationary {
                last_timestamp_ms, ..
            } => *last_timestamp_ms,
        };
        if let (Some(now), Some(prev)) = (curr_data.timestamp_ms, last_timestamp_ms) {
            if now < prev {
                return true;
            }
        };

        return false;
    }

    fn process_moving_data(
        last_point: &Point,
        last_timestamp_ms: Option<i64>,
        curr_data: &RawData,
    ) -> ProcessResult {
        const TIME_THRESHOLD_IN_MS: i64 = 5 * 1000;
        const SPEED_THRESHOLD: f64 = 250.0; // m/s

        let time_diff_in_ms = match (curr_data.timestamp_ms, last_timestamp_ms) {
            (Some(now), Some(prev)) => Some(now - prev),
            (None, _) | (_, None) => None,
        };

        let time_based_result = {
            match time_diff_in_ms {
                None => ProcessResult::Append,
                Some(diff_in_ms) => {
                    if diff_in_ms > TIME_THRESHOLD_IN_MS {
                        ProcessResult::NewSegment
                    } else {
                        ProcessResult::Append
                    }
                }
            }
        };

        if time_based_result == ProcessResult::Append {
            // let's consider (speed) distance now
            let time_in_sec = time_diff_in_ms.unwrap_or(TIME_THRESHOLD_IN_MS) as f64 / 1000.0;
            let speed = curr_data.point.haversine_distance(&last_point) / time_in_sec.max(0.01);
            if speed < SPEED_THRESHOLD {
                ProcessResult::Append
            } else {
                ProcessResult::NewSegment
            }
        } else {
            time_based_result
        }
    }

    pub fn preprocess(&mut self, curr_data: &RawData) -> ProcessResult {
        // Something to note:
        // * Accuracy is not well defined. The unit is meters but: On android,
        //  it is the radius of this location at the 68th percentile confidence
        //  level. On iOS, it is not specified in document. It seems the
        //  accuaracy is always poor (higher in value) on iOS, maybe it is using
        //  95th percentile. I am not use, no one is normalizing this value, we
        //  might need to use different threshold and tune it ourselves.
        //
        // * Values in GPX file are just from the device and we lose the device
        //   info (According to the bahvior of Guru Map). So we might need to
        //   use the iOS threshold or tune a new one. I am not sure. :(
        use GpsPreprocessorState::*;

        // TODO: need to tune this
        const DISTANCE_THRESHOLD_FOR_ENDING_STATIONARY_IN_M: f64 = 0.;
        const DISTANCE_THRESHOLD_FOR_BEGINING_STATIONARY_IN_M: f64 = f64::MAX / 2.;
        const TIME_TO_WAIT_BEFORE_BEGINING_STATIONARY_IN_MS: i64 = i64::MAX / 2;
        const FALLBACK_NUM_OF_DATA_TO_WAIT_BEFORE_BEGINING_STATIONARY: i64 = i64::MAX / 2;

        // We don't update our state if the data is bad.
        if self.is_bad_data(curr_data) {
            return ProcessResult::Ignore;
        };

        let start_moving = |curr_data: &RawData| Moving {
            last_point: curr_data.point.clone(),
            last_timestamp_ms: curr_data.timestamp_ms,
            possible_center_point: curr_data.point.clone(),
            timestamp_ms_when_center_point_picked: curr_data.timestamp_ms,
            num_of_data_since_center_point_picked: 0,
        };

        match &mut self.state {
            Empty => {
                self.state = start_moving(curr_data);
                ProcessResult::NewSegment
            }
            Moving {
                last_point,
                last_timestamp_ms,
                possible_center_point,
                timestamp_ms_when_center_point_picked,
                num_of_data_since_center_point_picked,
            } => {
                let result = Self::process_moving_data(last_point, *last_timestamp_ms, curr_data);
                *last_point = curr_data.point.clone();
                *last_timestamp_ms = curr_data.timestamp_ms;

                // consider if we need to become stationary
                if curr_data.point.haversine_distance(possible_center_point)
                    <= DISTANCE_THRESHOLD_FOR_BEGINING_STATIONARY_IN_M
                {
                    *num_of_data_since_center_point_picked += 1;
                    let should_become_stationary = if let (Some(now), Some(prev)) = (
                        curr_data.timestamp_ms,
                        *timestamp_ms_when_center_point_picked,
                    ) {
                        prev + TIME_TO_WAIT_BEFORE_BEGINING_STATIONARY_IN_MS <= now
                    } else {
                        // we only fallback to counting in this case
                        *num_of_data_since_center_point_picked
                            >= FALLBACK_NUM_OF_DATA_TO_WAIT_BEFORE_BEGINING_STATIONARY
                    };
                    if should_become_stationary {
                        self.state = Stationary {
                            center_point: possible_center_point.clone(),
                            last_timestamp_ms: curr_data.timestamp_ms,
                        }
                    }
                } else {
                    // picking new center point
                    // TODO: maybe picking the middle point between the previous possible center point and the current
                    // point is better. I am not sure.
                    *possible_center_point = curr_data.point.clone();
                    *timestamp_ms_when_center_point_picked = curr_data.timestamp_ms;
                    *num_of_data_since_center_point_picked = 0;
                }

                result
            }
            Stationary {
                center_point,
                last_timestamp_ms,
            } => {
                if curr_data.point.haversine_distance(center_point)
                    <= DISTANCE_THRESHOLD_FOR_ENDING_STATIONARY_IN_M
                {
                    *last_timestamp_ms = curr_data.timestamp_ms;
                    ProcessResult::Ignore
                } else {
                    // ending stationary
                    let result =
                        Self::process_moving_data(center_point, *last_timestamp_ms, curr_data);
                    self.state = start_moving(curr_data);
                    result
                }
            }
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
