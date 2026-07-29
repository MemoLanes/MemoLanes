use crate::{
    gps_processor::{Point, RawData},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};

// Fill gaps in raw data to produce a smooth `JourneyVector`.
//
// Original points are always preserved. Interpolated points are inserted at
// `STEP_LENGTH` boundaries along the cumulative distance of each segment.
pub fn process(raw_data: &[Vec<RawData>]) -> Option<JourneyVector> {
    const STEP_LENGTH: f64 = 1000.;

    let mut track_segments = Vec::new();

    for raw_data_segment in raw_data {
        // get points
        let points: Vec<Point> = raw_data_segment
            .iter()
            .map(|data| data.point.clone())
            .collect();
        for seg in PathInterpolator::split_trajectory_at_180(&points) {
            if let Some(result) = PathInterpolator::interpolate_one_seg(&seg, STEP_LENGTH) {
                track_segments.push(result)
            }
        }
    }

    if track_segments.is_empty() {
        None
    } else {
        Some(JourneyVector { track_segments })
    }
}

struct PathInterpolator {}

impl PathInterpolator {
    fn split_trajectory_at_180(trajectory: &[Point]) -> Vec<Vec<Point>> {
        if trajectory.len() < 2 {
            return vec![trajectory.to_vec()];
        }

        let mut segments = Vec::new();
        let mut current_segment = vec![trajectory[0].clone()];

        for i in 1..trajectory.len() {
            let point_s = trajectory[i - 1].clone();
            let point_e = trajectory[i].clone();
            let lon1 = point_s.longitude;
            let lon2 = point_e.longitude;

            // determine whether two points cross the 180° meridian.
            if PathInterpolator::crosses_180th_meridian(lon1, lon2) {
                // try to find the intersect point
                if let Some(intersection) =
                    PathInterpolator::find_180_intersection(&point_s, &point_e)
                {
                    // lon1==0 is a very danger boundary condition which means lon1 = 0, lon2 = 180
                    if lon1 < 0. {
                        // add -180 to last seg's end
                        current_segment.push(Point {
                            latitude: intersection,
                            longitude: -180.,
                        });

                        segments.push(current_segment);

                        // next seg begin at 180
                        current_segment = vec![Point {
                            latitude: intersection,
                            longitude: 180.,
                        }];
                    } else {
                        // add 180 to last seg's end
                        current_segment.push(Point {
                            latitude: intersection,
                            longitude: 180.,
                        });
                        segments.push(current_segment);

                        // next seg begin at -180
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

    // do interpolate for a track not pass the ±180
    fn interpolate_one_seg(source_data: &[Point], step_length: f64) -> Option<TrackSegment> {
        // compute distance between two neighbor point
        let distances: Vec<f64> = source_data
            .windows(2)
            .map(|data| data[0].haversine_distance(&data[1]))
            .collect();

        // compute distance to the start point as index
        let mut prefix_sums: Vec<f64> = distances
            .iter()
            .scan(0., |state, &num| {
                *state += num;
                Some(*state)
            })
            .collect();

        // add the start point's index----0
        prefix_sums.insert(0, 0.);

        let lats: Vec<f64> = source_data.iter().map(|x| x.latitude).collect();
        let lons: Vec<f64> = source_data.iter().map(|x| x.longitude).collect();

        let track_points =
            PathInterpolator::get_track_points(&prefix_sums, &lats, &lons, step_length);

        if track_points.is_empty() {
            None
        } else {
            Some(TrackSegment { track_points })
        }
    }

    // with the input of distance (indexOf Points),lats and lons, and the step_length of the result get the intepolate result
    fn get_track_points(
        distance: &[f64],
        lat: &[f64],
        lon: &[f64],
        step_length: f64,
    ) -> Vec<TrackPoint> {
        use splines::{Interpolation, Key, Spline};

        assert!(step_length > 0.0, "step_length must be bigger than zero!");

        if distance.len() != lat.len() || distance.len() != lon.len() || distance.is_empty() {
            return Vec::new();
        }

        let original_track_point = |index: usize| TrackPoint {
            latitude: lat[index],
            longitude: lon[index],
        };

        if distance.len() == 1 || distance.iter().any(|value| !value.is_finite()) {
            return (0..distance.len()).map(original_track_point).collect();
        }

        // Repeated source points have the same cumulative distance. Keep all of
        // them in the output, but only use one as a spline control point so the
        // spline never has duplicate keys.
        let mut unique_indices = vec![0];
        for index in 1..distance.len() {
            if distance[index] > distance[*unique_indices.last().unwrap()] {
                unique_indices.push(index);
            }
        }

        // There is no non-zero-length interval to fill.
        if unique_indices.len() == 1 {
            return (0..distance.len()).map(original_track_point).collect();
        }

        let combine = |a: f64, b: f64| Key::new(a, b, Interpolation::<_, f64>::CatmullRom);
        let mut vec_key_lat: Vec<Key<f64, f64>> = unique_indices
            .iter()
            .map(|&index| combine(distance[index], lat[index]))
            .collect();
        let mut vec_key_lon: Vec<Key<f64, f64>> = unique_indices
            .iter()
            .map(|&index| combine(distance[index], lon[index]))
            .collect();

        // Catmull-Rom needs one control point before and two after the
        // interpolation range.
        vec_key_lat.insert(0, Key::new(-step_length, lat[0], Interpolation::default()));
        vec_key_lon.insert(0, Key::new(-step_length, lon[0], Interpolation::default()));

        let end_distance = *distance.last().unwrap();
        vec_key_lat.push(Key::new(
            end_distance + step_length,
            *lat.last().unwrap(),
            Interpolation::default(),
        ));
        vec_key_lat.push(Key::new(
            end_distance + step_length * 2.,
            *lat.last().unwrap(),
            Interpolation::default(),
        ));
        vec_key_lon.push(Key::new(
            end_distance + step_length,
            *lon.last().unwrap(),
            Interpolation::default(),
        ));
        vec_key_lon.push(Key::new(
            end_distance + step_length * 2.,
            *lon.last().unwrap(),
            Interpolation::default(),
        ));

        let spline_lat = Spline::from_vec(vec_key_lat);
        let spline_lon = Spline::from_vec(vec_key_lon);
        let round_to_six_decimal_places = |num: f64| (num * 1000000.0).round() / 1000000.0;

        let mut track_points = Vec::new();
        track_points.push(original_track_point(0));

        for index in 0..distance.len() - 1 {
            let interval_start = distance[index];
            let interval_end = distance[index + 1];
            let mut sample_distance =
                (interval_start / step_length).floor() * step_length + step_length;

            // Sample strictly inside the interval. Its endpoints are the
            // original points and are appended without modification.
            while sample_distance < interval_end {
                if let (Some(latitude), Some(longitude)) = (
                    spline_lat.sample(sample_distance),
                    spline_lon.sample(sample_distance),
                ) {
                    track_points.push(TrackPoint {
                        latitude: round_to_six_decimal_places(latitude),
                        longitude: round_to_six_decimal_places(longitude),
                    });
                }
                sample_distance += step_length;
            }

            track_points.push(original_track_point(index + 1));
        }

        track_points
    }

    // judge if the two lons across the ±180
    fn crosses_180th_meridian(lon1: f64, lon2: f64) -> bool {
        // if lon1 and lon2 both at the same side return false
        if lon1 * lon2 >= 0. {
            return false;
        }
        // I'm sure that it has bug when the points are both in polar area
        // also if we want to process the polar's data, split the track when we cross high latitude (70° e.g.) is a good idea
        // then we will write other code to interpolate the polar's point
        let delta_lon = (lon1 - lon2).abs();

        delta_lon > 180.0
    }

    fn find_180_intersection(point1: &Point, point2: &Point) -> Option<f64> {
        const EPSILON: f64 = 1e-6;

        if point1.longitude.abs() >= 180. {
            return Some(point1.latitude);
        }
        if point2.longitude.abs() >= 180. {
            return Some(point2.latitude);
        }

        let (x1, y1, z1) = point1.to_cartesian();
        let (x2, y2, z2) = point2.to_cartesian();

        let nx = y1 * z2 - z1 * y2;
        let ny = z1 * x2 - x1 * z2;
        let nz = x1 * y2 - y1 * x2;

        let dir_x = ny * 0.0 - nz * 1.0;
        let dir_y = nz * 0.0 - nx * 0.0;
        let dir_z = nx * 1.0 - ny * 0.0;

        let a = dir_x * dir_x + dir_y * dir_y + dir_z * dir_z;
        if a.abs() < EPSILON * EPSILON {
            return None;
        }

        let t = 1.0 / a.sqrt();
        let (x_a, y_a, z_a) = (dir_x * t, dir_y * t, dir_z * t);
        let (x_b, y_b, z_b) = (dir_x * (-t), dir_y * (-t), dir_z * (-t));

        let point_a = Point::to_geographic(x_a, y_a, z_a);
        let point_b = Point::to_geographic(x_b, y_b, z_b);

        let dist_total = point1.haversine_distance(point2);

        let dist1 = point1.haversine_distance(&point_a);
        let dist2 = point_a.haversine_distance(point2);
        if (dist1 + dist2 - dist_total).abs() < EPSILON {
            return Some(point_a.latitude);
        }

        let dist1 = point1.haversine_distance(&point_b);
        let dist2 = point_b.haversine_distance(point2);
        if (dist1 + dist2 - dist_total).abs() < EPSILON {
            return Some(point_b.latitude);
        }

        None
    }
}

#[cfg(test)]
mod path_interpolator_tests {
    #[test]
    fn test_crossing() {
        // cross ±180
        assert!(super::PathInterpolator::crosses_180th_meridian(
            170.0, -170.0
        ));
        assert!(super::PathInterpolator::crosses_180th_meridian(
            175.0, -179.0
        ));

        // not cross ±180
        assert!(!super::PathInterpolator::crosses_180th_meridian(10.0, 20.0));
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            -170.0, -160.0
        ));
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            170.0, 175.0
        ));

        // one of the point is at ±180
        assert!(!super::PathInterpolator::crosses_180th_meridian(
            170.0, 180.0
        ));
        assert!(super::PathInterpolator::crosses_180th_meridian(
            -170.0, 180.0
        ));
    }
}
