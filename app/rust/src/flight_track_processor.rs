use crate::{
    gps_processor::{Point, RawData},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};

pub struct PathInterpolator {}

impl PathInterpolator {
    // main func to interpolate rawdata to a smooth journeyvetor
    pub fn process_flight_track(source_data: &[RawData]) -> JourneyVector {
        let mut track_segments = Vec::new();
        // interpolate step_length
        const STEP_LENGTH: f64 = 1000.;

        // get points
        let points: Vec<Point> = source_data.iter().map(|data| data.point.clone()).collect();

        // split_trajectory_at_180
        let segs = PathInterpolator::split_trajectory_at_180(&points);

        for seg in &segs {
            track_segments.push(PathInterpolator::interpolate_one_seg(seg, STEP_LENGTH));
        }
        JourneyVector { track_segments }
    }

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
    fn interpolate_one_seg(source_data: &[Point], step_length: f64) -> TrackSegment {
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
        // return a TrackSegment
        TrackSegment {
            track_points: PathInterpolator::get_track_points(
                &prefix_sums,
                &lats,
                &lons,
                step_length,
            ),
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

            // CatmullRom need [last Points,nowPoints,next one,next two] so we need ……

            // add a point before the start with index  -step_length
            vec_key_lat.insert(0, Key::new(-step_length, lat[0], Interpolation::default()));
            vec_key_lon.insert(0, Key::new(-step_length, lon[0], Interpolation::default()));

            // add two point after the end with index   last+step_length、 last+step_length*2
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

            // get sample points index
            let sample_points =
                PathInterpolator::generate_range(*distance.last().unwrap(), step_length);

            let round_to_two_decimal_places = |num: f64| (num * 1000000.0).round() / 1000000.0;

            // do sample to get result
            sample_points.iter().for_each(|num| {
                track_points.push(TrackPoint {
                    latitude: round_to_two_decimal_places(
                        spline_lat.sample(*num).unwrap_or_default(),
                    ),
                    longitude: round_to_two_decimal_places(
                        spline_lon.sample(*num).unwrap_or_default(),
                    ),
                })
            });

            track_points
        }
    }

    // generate a vec have numbers between 0 to end with a step_length
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
        if point1.longitude.abs() > 179.999_999_999 {
            return Some(point1.latitude);
        }
        if point2.longitude.abs() > 179.999_999_999 {
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
        if a.abs() < 1e-12 {
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
        if (dist1 + dist2 - dist_total).abs() < 1e-6 {
            return Some(point_a.latitude);
        }

        let dist1 = point1.haversine_distance(&point_b);
        let dist2 = point_b.haversine_distance(point2);
        if (dist1 + dist2 - dist_total).abs() < 1e-6 {
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
