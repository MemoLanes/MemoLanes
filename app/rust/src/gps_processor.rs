use crate::{
    journey_header::{JourneyHeader, JourneyType},
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

    fn is_bad_data(&mut self, curr_data: &RawData) -> bool {
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

pub struct GpsPreprocessor {
    state: GpsPreprocessorState,
    bad_data_detector: BadDataDetector,
}

impl GpsPreprocessor {
    pub fn new() -> Self {
        GpsPreprocessor {
            state: GpsPreprocessorState::Empty,
            bad_data_detector: BadDataDetector::new(),
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
        last_point: &Point,
        last_timestamp_ms: Option<i64>,
        curr_data: &RawData,
    ) -> ProcessResult {
        const TOO_CLOSE_DISTANCE_IN_M: f64 = 0.1;

        let distance_in_m = curr_data.point.haversine_distance(last_point);

        if distance_in_m <= TOO_CLOSE_DISTANCE_IN_M {
            ProcessResult::Ignore
        } else {
            let time_diff_in_ms = match (curr_data.timestamp_ms, last_timestamp_ms) {
                (Some(now), Some(prev)) => Some(now - prev),
                (None, _) | (_, None) => None,
            };

            if distance_in_m <= 1_000. {
                match time_diff_in_ms {
                    // don't have timestamp, just be conservative and append
                    None => ProcessResult::Append,
                    Some(time_diff_in_ms) => {
                        // more willing to connect two points if they are close
                        // in normal condition, we should have 1 data per sec
                        // we should mostly trust the data here and try to
                        // filter out bad ones in `BadDataDetector`.
                        let time_threshold_in_sec = if distance_in_m < 5. {
                            60 * 60 // 1h
                        } else if distance_in_m < 50. {
                            20 // 20 sec
                        } else {
                            4 // 4 sec
                        };
                        if time_diff_in_ms <= time_threshold_in_sec * 1000 {
                            ProcessResult::Append
                        } else {
                            ProcessResult::NewSegment
                        }
                    }
                }
            } else {
                // Too far, start a new segment
                ProcessResult::NewSegment
            }
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

        const DISTANCE_THRESHOLD_FOR_ENDING_STATIONARY_IN_M: f64 = 10.0;
        const DISTANCE_THRESHOLD_FOR_BEGINING_STATIONARY_IN_M: f64 = 5.0;
        const TIME_TO_WAIT_BEFORE_BEGINING_STATIONARY_IN_MS: i64 = 60 * 1000;
        const FALLBACK_NUM_OF_DATA_TO_WAIT_BEFORE_BEGINING_STATIONARY: i64 = 60;
        const DEFAULT_ACCURACY_OF_POINT: f32 = 30.0;

        // We don't update our state if the data is bad.
        if self.bad_data_detector.is_bad_data(curr_data) {
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
pub struct PathInterpolator {
    pub step_length: f64,
}

impl PathInterpolator {
    pub fn new() -> Self {
        PathInterpolator { step_length: 1000. }
    }

    //generate a vec have numbers between 0 to end with a step_length
    fn generate_range(end: f64, step_length: f64) -> Vec<f64> {
        if step_length <= 0.0 {
            panic!("step_length must bigger than zero!");
        }

        let mut result = Vec::new();
        let mut current = 0.0;

        while current <= end + 1e-10 {
            result.push(current);
            current += step_length;
        }
        result.push(end);
        result
    }

    // with the input of distance (indexOf Points),lats and lons, and the step_length of the result get the intepolate result
    fn get_track_points(
        distance: &Vec<f64>,
        lat: &Vec<f64>,
        lon: &Vec<f64>,
        step_length: f64,
    ) -> Vec<TrackPoint> {
        use splines::{Interpolation, Key, Spline};
        let mut track_points = Vec::new();

        if distance.len() < 2 || lat.len() < 2 || lon.len() < 2 {
            track_points
        } else {
            let combine = |a: f64, b: f64| Key::new(a, b, Interpolation::<_, f64>::CatmullRom);

            let mut vec_key_lat: Vec<Key<f64, f64>> = distance
                .iter()
                .zip(lat.iter())
                .map(|(a, b)| combine(*a, *b))
                .collect();

            let mut vec_key_lon: Vec<Key<f64, f64>> = distance
                .iter()
                .zip(lon.iter())
                .map(|(a, b)| combine(*a, *b))
                .collect();

            //CatmullRom need nowPoints  last Points and next two Points so we need ……

            //add a point before the start with index  -step_length
            vec_key_lat.insert(0, Key::new(-step_length, lat[0], Interpolation::default()));
            vec_key_lon.insert(0, Key::new(-step_length, lon[0], Interpolation::default()));

            //add two point after the end with index   last+step_length、 last+step_length*2
            vec_key_lat.push(Key::new(
                distance.last().unwrap() + step_length,
                *lat.last().unwrap(),
                Interpolation::default(),
            ));
            vec_key_lat.push(Key::new(
                distance.last().unwrap() + step_length * 2.,
                *lat.last().unwrap(),
                Interpolation::default(),
            ));
            vec_key_lon.push(Key::new(
                distance.last().unwrap() + step_length,
                *lon.last().unwrap(),
                Interpolation::default(),
            ));
            vec_key_lon.push(Key::new(
                distance.last().unwrap() + step_length * 2.,
                *lon.last().unwrap(),
                Interpolation::default(),
            ));

            let spline_lat = Spline::from_vec(vec_key_lat);
            let spline_lon = Spline::from_vec(vec_key_lon);

            //get sample points index
            let sample_points =
                PathInterpolator::generate_range(*distance.last().unwrap(), step_length);
            //do sample to get result
            sample_points.iter().for_each(|num| {
                track_points.push(TrackPoint {
                    latitude: spline_lat.sample(*num).unwrap_or_default(),
                    longitude: spline_lon.sample(*num).unwrap_or_default(),
                })
            });

            track_points
        }
    }

    //do interpolate for a track not pass the ±180
    fn interpolate_one_seg(source_data: &Vec<Point>, step_length: f64) -> TrackSegment {
        //compute distance between two neighbor point
        let distances: Vec<f64> = source_data
            .windows(2)
            .map(|data| data[0].haversine_distance(&data[1]))
            .collect();

        //compute distance to the start point as index
        let mut prefix_sums: Vec<f64> = distances
            .iter()
            .scan(0., |state, &num| {
                *state = *state + num; //
                Some(*state) //
            })
            .collect();

        //add the start point's index----0
        prefix_sums.insert(0, 0.);

        let lats = source_data.iter().map(|x| x.latitude).collect();
        let lons = source_data.iter().map(|x| x.longitude).collect();
        //return a TrackSegment
        TrackSegment {
            track_points: PathInterpolator::get_track_points(
                &prefix_sums,
                &lats,
                &lons,
                step_length,
            ),
        }
    }

    // main func to interpolate rawdata to a smooth journeyvetor
    pub fn interpolate(&self, source_data: &Vec<RawData>) -> JourneyVector {
        let mut track_segments = Vec::new();

        //get points
        let points: Vec<Point> = source_data.iter().map(|data| data.point.clone()).collect();

        //split_trajectory_at_180
        let segs = PathInterpolator::split_trajectory_at_180(&points);

        for seg in &segs {
            track_segments.push(PathInterpolator::interpolate_one_seg(seg, self.step_length));
        }
        JourneyVector { track_segments }
    }

    //judge if the two lons across the ±180
    pub fn crosses_180th_meridian(lon1: f64, lon2: f64) -> bool {
        //if lon1 and lon2 both at the same side return false
        if lon1 * lon2 >= 0. {
            return false;
        }
        //I'm sure that it has bug when the points are both in polar area
        //also if we want to process the polar's data, split the track when we cross high latitude (70° e.g.) is a good idea
        //then we will write other code to interpolate the polar's point
        let delta_lon = (lon1 - lon2).abs();

        delta_lon > 180.0
    }

    //split_trajectory_at_180
    fn split_trajectory_at_180(trajectory: &Vec<Point>) -> Vec<Vec<Point>> {
        if trajectory.len() < 2 {
            return vec![trajectory.to_vec()];
        }

        let mut segments = Vec::new();
        let mut current_segment = vec![trajectory[0].clone()];

        for i in 1..trajectory.len() {
            let lon1 = trajectory[i - 1].longitude;
            let lat1 = trajectory[i - 1].latitude;
            let lon2 = trajectory[i].longitude;
            let lat2 = trajectory[i].latitude;

            // determine whether two points cross the 180° meridian.
            if PathInterpolator::crosses_180th_meridian(lon1, lon2) {
                // try to find the intersect point
                if let Some(intersection) =
                    PathInterpolator::find_180_intersection(lon1, lat1, lon2, lat2)
                {
                    //lon1==0 is a very danger boundary condition which means lon1 = 0, lon2 = 180
                    if lon1 < 0. {
                        //add -180 to last seg's end
                        current_segment.push(Point {
                            latitude: intersection,
                            longitude: -180.,
                        });

                        segments.push(current_segment);

                        //next seg begin at 180
                        current_segment = vec![Point {
                            latitude: intersection,
                            longitude: 180.,
                        }];
                    } else {
                        //add 180 to last seg's end
                        current_segment.push(Point {
                            latitude: intersection,
                            longitude: 180.,
                        });
                        segments.push(current_segment);

                        //next seg begin at -180
                        current_segment = vec![Point {
                            latitude: intersection,
                            longitude: -180.,
                        }];
                    }
                }
            }

            current_segment.push(trajectory[i].clone());
        }

        if !current_segment.is_empty() {
            segments.push(current_segment);
        }

        segments
    }

    fn to_radians(deg: f64) -> f64 {
        use std::f64::consts::PI;
        deg * PI / 180.0
    }
    fn to_degrees(rad: f64) -> f64 {
        use std::f64::consts::PI;
        rad * 180.0 / PI
    }

    fn to_cartesian(lon: f64, lat: f64) -> (f64, f64, f64) {
        let lon_rad = PathInterpolator::to_radians(lon);
        let lat_rad = PathInterpolator::to_radians(lat);
        let x = lat_rad.cos() * lon_rad.cos();
        let y = lat_rad.cos() * lon_rad.sin();
        let z = lat_rad.sin();
        (x, y, z)
    }

    fn normalize_longitude(mut lon: f64) -> f64 {
        while lon >= 180.0 {
            lon -= 360.0;
        }
        while lon < -180.0 {
            lon += 360.0;
        }
        lon
    }

    // some code possibly redundant code // with KM rtn
    fn haversine_distance(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;
        let lon1 = PathInterpolator::to_radians(lon1);
        let lat1 = PathInterpolator::to_radians(lat1);
        let lon2 = PathInterpolator::to_radians(lon2);
        let lat2 = PathInterpolator::to_radians(lat2);

        let dlon = lon2 - lon1;
        let dlat = lat2 - lat1;

        let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_KM * c
    }

    fn to_geographic(x: f64, y: f64, z: f64) -> (f64, f64) {
        let lon = PathInterpolator::to_degrees(y.atan2(x));
        let lat = PathInterpolator::to_degrees(z.atan2((x * x + y * y).sqrt()));
        (PathInterpolator::normalize_longitude(lon), lat)
    }

    fn find_180_intersection(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> Option<f64> {
        if lon1.abs() > 179.999_999_999 {
            return Some(lat1);
        }
        if lon2.abs() > 179.999_999_999 {
            return Some(lat2);
        }

        let (x1, y1, z1) = PathInterpolator::to_cartesian(lon1, lat1);
        let (x2, y2, z2) = PathInterpolator::to_cartesian(lon2, lat2);

        let nx = y1 * z2 - z1 * y2;
        let ny = z1 * x2 - x1 * z2;
        let nz = x1 * y2 - y1 * x2;

        let dir_x = ny * 0.0 - nz * 1.0;
        let dir_y = nz * 0.0 - nx * 0.0;
        let dir_z = nx * 1.0 - ny * 0.0;

        let a = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
        if a.abs() < 1e-12 {
            return None;
        }

        let t = 1.0 / a.sqrt();
        let (x_a, y_a, z_a) = (dir_x * t, dir_y * t, dir_z * t);
        let (x_b, y_b, z_b) = (dir_x * (-t), dir_y * (-t), dir_z * (-t));

        let (lon_a, lat_a) = PathInterpolator::to_geographic(x_a, y_a, z_a);
        let (lon_b, lat_b) = PathInterpolator::to_geographic(x_b, y_b, z_b);

        let dist_total = PathInterpolator::haversine_distance(lon1, lat1, lon2, lat2);

        let dist1 = PathInterpolator::haversine_distance(lon1, lat1, lon_a, lat_a);
        let dist2 = PathInterpolator::haversine_distance(lon_a, lat_a, lon2, lat2);
        if (dist1 + dist2 - dist_total).abs() < 1e-6 {
            return Some(lat_a);
        }

        let dist1 = PathInterpolator::haversine_distance(lon1, lat1, lon_b, lat_b);
        let dist2 = PathInterpolator::haversine_distance(lon_b, lat_b, lon2, lat2);
        if (dist1 + dist2 - dist_total).abs() < 1e-6 {
            return Some(lat_b);
        }

        None
    }
}
#[cfg(test)]
mod path_interpolator_tests {

    #[test]
    fn test_crossing() {
        // 跨越180°经线的情况
        assert!(super::PathInterpolator::crosses_180th_meridian(
            170.0, -170.0
        ));
        assert!(super::PathInterpolator::crosses_180th_meridian(
            175.0, -179.0
        ));

        // 不跨越180°经线的情况
        assert!(!super::PathInterpolator::crosses_180th_meridian(10.0, 20.0));
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            -170.0, -160.0
        ));
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            170.0, 175.0
        ));

        // 边界情况 - 其中一点在180°经线上
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            170.0, 180.0
        ));
        assert!(super::PathInterpolator::crosses_180th_meridian(
            -170.0, 180.0
        ));
    }
}
