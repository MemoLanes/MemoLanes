use crate::{
    gps::RawGPSPoint,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    spherical_geometry::{GreatCircleArc, UnitPoint, EARTH_RADIUS_METERS},
};

const STEP_LENGTH_METERS: f64 = 1_000.0;

/// Smooth sparse flight observations on the sphere, then sample the curve.
/// Original observations set the smoothing scale and survive unchanged;
/// generated samples never become additional spline controls.
pub fn process(raw_data: &[Vec<RawGPSPoint>]) -> Option<JourneyVector> {
    let mut track_segments = Vec::new();
    for source in raw_data {
        let mut points = Vec::new();
        // Invalid observations survive unchanged, but cannot influence a spline
        // on either side. Ordinary input takes a single valid run.
        for run in source.chunk_by(|a, b| valid(a) == valid(b)) {
            if valid(&run[0]) {
                points.extend(interpolate(run));
            } else {
                points.extend(run.iter().map(to_track_point));
            }
        }
        track_segments.extend(split_at_dateline(points));
    }
    (!track_segments.is_empty()).then_some(JourneyVector { track_segments })
}

fn valid(data: &RawGPSPoint) -> bool {
    data.point.longitude.is_finite()
        && data.point.latitude.is_finite()
        && (-90.0..=90.0).contains(&data.point.latitude)
}

fn to_track_point(data: &RawGPSPoint) -> TrackPoint {
    TrackPoint {
        latitude: data.point.latitude,
        longitude: data.point.longitude,
    }
}

struct ControlPoint {
    distance: f64,
    position: UnitPoint,
    tangent: UnitPoint,
}

fn interpolate(source: &[RawGPSPoint]) -> Vec<TrackPoint> {
    let zero = UnitPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut controls = vec![ControlPoint {
        distance: 0.0,
        position: UnitPoint::from_lng_lat(source[0].point.longitude, source[0].point.latitude),
        tangent: zero,
    }];
    let mut source_distances = vec![0.0];
    let mut distance = 0.0;
    for pair in source.windows(2) {
        let arc = GreatCircleArc::new(
            UnitPoint::from_lng_lat(pair[0].point.longitude, pair[0].point.latitude),
            UnitPoint::from_lng_lat(pair[1].point.longitude, pair[1].point.latitude),
        );
        let length = arc.angle * EARTH_RADIUS_METERS;
        // Repeated observations remain in the output, but must not create
        // duplicate spline parameters or divide a tangent by zero.
        if length > 1e-6 {
            distance += length;
            controls.push(ControlPoint {
                distance,
                position: arc.at(1.0),
                tangent: zero,
            });
        }
        source_distances.push(distance);
    }
    if controls.len() == 1 {
        return source.iter().map(to_track_point).collect();
    }
    for i in 0..controls.len() {
        let current = &controls[i];
        let previous = i.checked_sub(1).map(|prev| &controls[prev]);
        let next = controls.get(i + 1);
        // Match main's distance-parameterized Catmull-Rom tangents. Repeated
        // endpoint controls sit one sample interval outside the observed path.
        let (a, b, endpoint_padding) = match (previous, next) {
            (Some(a), Some(b)) => (a, b, 0.0),
            (None, Some(b)) => (current, b, STEP_LENGTH_METERS),
            (Some(a), None) => (a, current, STEP_LENGTH_METERS),
            (None, None) => unreachable!(),
        };
        let scale = EARTH_RADIUS_METERS / (b.distance - a.distance + endpoint_padding);
        let tangent = b.position.scale_add(scale, a.position, -scale);
        // Share one tangent across both sides of each observation, projected
        // onto the sphere. This is what removes the piecewise-arc hard turn.
        controls[i].tangent =
            tangent.scale_add(1.0, current.position, -tangent.dot(current.position));
    }

    // Two distinct observations provide no turning information. Keep their
    // shortest arc, including the deterministic antipodal case.
    let direct_arc = match controls.as_slice() {
        [a, b] => Some(GreatCircleArc::new(a.position, b.position)),
        _ => None,
    };
    let mut points = vec![to_track_point(&source[0])];
    let mut control = 0;
    for (interval, observation) in source_distances.windows(2).zip(&source[1..]) {
        let mut sample =
            (interval[0] / STEP_LENGTH_METERS).floor() * STEP_LENGTH_METERS + STEP_LENGTH_METERS;
        while sample < interval[1] {
            while controls[control + 1].distance < sample {
                control += 1;
            }
            let a = &controls[control];
            let b = &controls[control + 1];
            let t = (sample - a.distance) / (b.distance - a.distance);
            let point = match direct_arc {
                Some(arc) => to_interpolated_point(arc.at(t)),
                None => interpolate_controls(a, b, t),
            };
            append_with_spacing(&mut points, point);
            sample += STEP_LENGTH_METERS;
        }
        append_with_spacing(&mut points, to_track_point(observation));
    }
    points
}

fn interpolate_controls(a: &ControlPoint, b: &ControlPoint, t: f64) -> TrackPoint {
    let t2 = t * t;
    let t3 = t2 * t;
    let angle = (b.distance - a.distance) / EARTH_RADIUS_METERS;
    // Cubic Hermite in Cartesian coordinates, normalized onto the sphere.
    // Observations following one great circle keep the curve in its plane.
    let start = a.position.scale_add(
        2.0 * t3 - 3.0 * t2 + 1.0,
        a.tangent,
        (t3 - 2.0 * t2 + t) * angle,
    );
    let end = b
        .position
        .scale_add(-2.0 * t3 + 3.0 * t2, b.tangent, (t3 - t2) * angle);
    let position = start.scale_add(1.0, end, 1.0);
    // Opposite controls can cancel in Cartesian coordinates. The spherical
    // arc still provides a defined position for this degenerate configuration.
    if position.dot(position) < 1e-12 {
        to_interpolated_point(GreatCircleArc::new(a.position, b.position).at(t))
    } else {
        to_interpolated_point(position.normalized())
    }
}

fn to_interpolated_point(position: UnitPoint) -> TrackPoint {
    let (longitude, latitude) = position.to_lng_lat();
    let round = |coordinate: f64| (coordinate * 1_000_000.0).round() / 1_000_000.0;
    TrackPoint {
        longitude: round(longitude),
        latitude: round(latitude),
    }
}

fn append_with_spacing(points: &mut Vec<TrackPoint>, point: TrackPoint) {
    let previous = points.last().unwrap();
    let arc = GreatCircleArc::new(
        UnitPoint::from_lng_lat(previous.longitude, previous.latitude),
        UnitPoint::from_lng_lat(point.longitude, point.latitude),
    );
    // Spline speed can exceed the distance parameter locally. Fill any extra
    // gap so smoothing does not weaken the one-kilometre spacing guarantee.
    let distance = arc.angle * EARTH_RADIUS_METERS;
    if distance > STEP_LENGTH_METERS + 0.1 {
        let count = (distance / STEP_LENGTH_METERS).ceil() as usize;
        for i in 1..count {
            points.push(to_interpolated_point(arc.at(i as f64 / count as f64)));
        }
    }
    points.push(point);
}

// Planar consumers (notably box editing) must never see a longitude jump across
// the map. Split after sampling so source points and the sampling cadence survive.
fn split_at_dateline(points: Vec<TrackPoint>) -> Vec<TrackSegment> {
    let mut segments = Vec::new();
    let mut current: Vec<TrackPoint> = Vec::new();
    for point in points {
        if let Some(previous) = current.last() {
            if (point.longitude - previous.longitude).abs() > 180.0 {
                let longitude = 180.0_f64.copysign(previous.longitude);
                let latitude = dateline_latitude(previous, &point);
                // Reuse original boundary points rather than replacing them or
                // adding a second copy on the same side of the split.
                if previous.longitude != longitude {
                    current.push(TrackPoint {
                        latitude,
                        longitude,
                    });
                }
                segments.push(TrackSegment {
                    track_points: std::mem::take(&mut current),
                });
                if point.longitude != -longitude {
                    current.push(TrackPoint {
                        latitude,
                        longitude: -longitude,
                    });
                }
            }
        }
        current.push(point);
    }
    if !current.is_empty() {
        segments.push(TrackSegment {
            track_points: current,
        });
    }
    segments
}

fn dateline_latitude(a: &TrackPoint, b: &TrackPoint) -> f64 {
    if a.longitude.abs() == 180.0 {
        return a.latitude;
    }
    if b.longitude.abs() == 180.0 {
        return b.latitude;
    }
    let a = UnitPoint::from_lng_lat(a.longitude, a.latitude);
    let b = UnitPoint::from_lng_lat(b.longitude, b.latitude);
    // The chord and the short great-circle arc lie in the same plane through
    // the origin. Intersect the chord with y = 0, then project back to latitude.
    // For a dateline crossing x is negative; no vector normalization is needed.
    let t = a.y / (a.y - b.y);
    let x = a.x + t * (b.x - a.x);
    let z = a.z + t * (b.z - a.z);
    z.atan2(-x).to_degrees()
}
