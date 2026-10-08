use crate::{
    gps::{Point, RawGPSPoint},
    journey_date_picker::JourneyDatePicker,
    journey_header::{JourneyHeader, JourneyType},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};
use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::DateTime;

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

// It is unfortunate that we may keep duplicate data but I believe this is
// clearer and easier to maintain.
struct BadDataDetector {
    timestamp_ms_and_point: Option<(i64, Point)>,
    speed: Option<f32>,
}

impl BadDataDetector {
    pub fn new() -> Self {
        BadDataDetector {
            timestamp_ms_and_point: None,
            speed: None,
        }
    }

    fn is_bad_data(&mut self, curr_data: &RawGPSPoint) -> bool {
        const ACCURACY_THRESHOLD: f32 = 50.;
        const ACCELERATION_THRESHOLD: f32 = 10.;
        // We mostly don't care deceleration, but just in case we had a very bad
        // data that bring the speed down a lot.
        const DECELERATION_THRESHOLD: f32 = -20.;

        if let Some(accuracy) = curr_data.accuracy {
            if accuracy > ACCURACY_THRESHOLD {
                return true;
            }
        }

        if let Some(timestamp_ms) = curr_data.timestamp_ms {
            if let Some((prev_timestamp_ms, prev_point)) = &self.timestamp_ms_and_point {
                // Invalid timestamp
                if timestamp_ms <= *prev_timestamp_ms {
                    return true;
                }
                let time_span_in_sec = (timestamp_ms - prev_timestamp_ms) as f32 / 1000.0;

                // reset the state if the time span is too long, otherwise we
                // might have a very outdated speed value. In general, a lot of
                // assumptions here do not hold if the time span is too long.
                if time_span_in_sec >= 10. {
                    self.timestamp_ms_and_point = None;
                    self.speed = None;
                    return false;
                }

                // computing the speed by ourself instead of using the speed from `RawData`.
                let distance_m = curr_data.point.haversine_distance(prev_point) as f32;
                let speed = distance_m / time_span_in_sec;

                if let Some(last_speed) = self.speed {
                    let acceleration = (speed - last_speed) / time_span_in_sec;
                    // We only care about acceleration, not deceleration.
                    // Maybe we should also consider direction.
                    if !(DECELERATION_THRESHOLD..=ACCELERATION_THRESHOLD).contains(&acceleration) {
                        return true;
                    }
                }

                // Note: state should only be updated for good data
                self.speed = Some(speed);
            }
            self.timestamp_ms_and_point = Some((timestamp_ms, curr_data.point.clone()));
        }
        false
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

struct SegmentGapThreshold {
    distance_m: f64,
    max_gap_sec: i64,
}

#[derive(Clone, Copy, Debug)]
pub enum SegmentGapRule {
    Default,
    Spare,
}

pub struct GpsPreprocessor {
    state: GpsPreprocessorState,
    bad_data_detector: BadDataDetector,
    rule: SegmentGapRule,
}

impl GpsPreprocessor {
    pub fn new() -> Self {
        Self::new_with_rule(SegmentGapRule::Default)
    }

    pub fn new_with_rule(rule: SegmentGapRule) -> Self {
        Self {
            state: GpsPreprocessorState::Empty,
            bad_data_detector: BadDataDetector::new(),
            rule,
        }
    }

    pub fn last_kept_point(&self) -> Option<Point> {
        use GpsPreprocessorState::*;
        match &self.state {
            Empty => None,
            Moving {
                last_point: point, ..
            }
            | Stationary {
                center_point: point,
                ..
            } => Some(point.clone()),
        }
    }

    fn process_moving_data(
        rule: SegmentGapRule,
        last_point: &Point,
        last_timestamp_ms: Option<i64>,
        curr_data: &RawGPSPoint,
    ) -> ProcessResult {
        // Rules must be ordered by `distance_m` in ascending order.
        // The first matching rule is applied.
        type SegmentGapProfile = &'static [SegmentGapThreshold; 3];
        const DEFAULT_SEGMENT_GAP_RULES: SegmentGapProfile = &[
            SegmentGapThreshold {
                distance_m: 5.0,
                max_gap_sec: 3600,
            },
            SegmentGapThreshold {
                distance_m: 50.0,
                max_gap_sec: 20,
            },
            SegmentGapThreshold {
                distance_m: 1000.0,
                max_gap_sec: 4,
            },
        ];
        const SPARE_SEGMENT_GAP_RULES: SegmentGapProfile = &[
            SegmentGapThreshold {
                distance_m: 5.0,
                max_gap_sec: 3600,
            },
            SegmentGapThreshold {
                distance_m: 150.0,
                max_gap_sec: 240,
            },
            SegmentGapThreshold {
                distance_m: 1000.0,
                max_gap_sec: 120,
            },
        ];

        const TOO_CLOSE_DISTANCE_IN_M: f64 = 0.1;

        let distance_in_m = curr_data.point.haversine_distance(last_point);

        if distance_in_m <= TOO_CLOSE_DISTANCE_IN_M {
            ProcessResult::Ignore
        } else {
            let time_diff_in_ms = match (curr_data.timestamp_ms, last_timestamp_ms) {
                (Some(now), Some(prev)) => Some(now - prev),
                (None, _) | (_, None) => None,
            };

            match time_diff_in_ms {
                // don't have timestamp, just be conservative and append
                None => ProcessResult::Append,
                Some(time_diff_in_ms) => {
                    // more willing to connect two points if they are close
                    // in normal condition, we should have 1 data per sec
                    // we should mostly trust the data here and try to
                    // filter out bad ones in `BadDataDetector`.
                    for rule in match rule {
                        SegmentGapRule::Default => DEFAULT_SEGMENT_GAP_RULES,
                        SegmentGapRule::Spare => SPARE_SEGMENT_GAP_RULES,
                    }
                    .iter()
                    {
                        if distance_in_m <= rule.distance_m {
                            return if time_diff_in_ms <= rule.max_gap_sec * 1000 {
                                ProcessResult::Append
                            } else {
                                ProcessResult::NewSegment
                            };
                        }
                    }
                    // Too far, start a new segment
                    ProcessResult::NewSegment
                }
            }
        }
    }

    pub fn preprocess(&mut self, curr_data: &RawGPSPoint) -> ProcessResult {
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

        const DISTANCE_THRESHOLD_FOR_ENDING_STATIONARY_IN_M: f64 = 10.0;
        const DISTANCE_THRESHOLD_FOR_BEGINING_STATIONARY_IN_M: f64 = 5.0;
        const TIME_TO_WAIT_BEFORE_BEGINING_STATIONARY_IN_MS: i64 = 60 * 1000;
        const FALLBACK_NUM_OF_DATA_TO_WAIT_BEFORE_BEGINING_STATIONARY: i64 = 60;
        const DEFAULT_ACCURACY_OF_POINT: f32 = 30.0;

        // We don't update our state if the data is bad.
        if self.bad_data_detector.is_bad_data(curr_data) {
            return ProcessResult::Ignore;
        };

        let start_moving = |curr_data: &RawGPSPoint| Moving {
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
                let result =
                    Self::process_moving_data(self.rule, last_point, *last_timestamp_ms, curr_data);
                if result != ProcessResult::Ignore {
                    *last_point = curr_data.point.clone();
                    *last_timestamp_ms = curr_data.timestamp_ms;
                }

                let accuracy = curr_data.accuracy.unwrap_or(DEFAULT_ACCURACY_OF_POINT);

                // consider if we need to become stationary
                // here use the accuracy of gps as threshold
                if curr_data.point.haversine_distance(possible_center_point)
                    <= ((accuracy).min(DEFAULT_ACCURACY_OF_POINT)) as f64
                        + DISTANCE_THRESHOLD_FOR_BEGINING_STATIONARY_IN_M
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
                            center_point: curr_data.point.clone(),
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
                //use accuracy as threshold of break stationary state
                //center_point to compute distance
                //last_point to compute acceleration
                let distance = curr_data.point.haversine_distance(center_point);
                let accuracy = curr_data.accuracy.unwrap_or(DEFAULT_ACCURACY_OF_POINT);
                if distance <= (accuracy) as f64 + DISTANCE_THRESHOLD_FOR_ENDING_STATIONARY_IN_M {
                    *last_timestamp_ms = curr_data.timestamp_ms;
                    ProcessResult::Ignore
                } else {
                    //then ending stationary change to move mode
                    let result = Self::process_moving_data(
                        self.rule,
                        center_point,
                        *last_timestamp_ms,
                        curr_data,
                    );
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

#[auto_context]
pub fn build_journey_vector(
    results: impl Iterator<Item = Result<PreprocessedData>>,
    mut journey_date_picker: Option<&mut JourneyDatePicker>,
) -> Result<Option<JourneyVector>> {
    let mut segments = Vec::new();
    let mut current_segment = Vec::new();

    for result in results {
        let data = result?;
        let need_break = data.process_result == ProcessResult::NewSegment;
        if need_break && !current_segment.is_empty() {
            segments.push(TrackSegment {
                track_points: current_segment,
            });
            current_segment = Vec::new();
        }
        if data.process_result != ProcessResult::Ignore {
            if let Some(journey_date_picker) = journey_date_picker.as_mut() {
                if let Some(time) = data
                    .timestamp_sec
                    .map(|x| DateTime::from_timestamp(x, 0).unwrap())
                {
                    journey_date_picker.add_point(time, &data.track_point);
                }
            }
            current_segment.push(data.track_point);
        }
    }
    if !current_segment.is_empty() {
        segments.push(TrackSegment {
            track_points: current_segment,
        });
    }

    if segments.is_empty() {
        Ok(None)
    } else {
        Ok(Some(JourneyVector {
            track_segments: segments,
        }))
    }
}

pub struct GpsPostprocessor {}

impl GpsPostprocessor {
    pub fn process(journey_vector: JourneyVector) -> JourneyVector {
        journey_vector
    }

    pub fn current_algo() -> String {
        "0".to_string()
    }

    // When introducing a new algorithm, remember to update the
    // `currentOptimizationCheckVersion` on the flutter side
    pub fn outdated_algo(journey_header: &JourneyHeader) -> bool {
        match journey_header.journey_type {
            JourneyType::Bitmap => false,
            #[allow(clippy::redundant_pattern_matching)]
            JourneyType::Vector => match journey_header.postprocessor_algo {
                None => true,
                Some(_) => false,
            },
        }
    }
}
