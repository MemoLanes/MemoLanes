use crate::{
    gps::RawGPSPoint,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    spherical_geometry::{GreatCircleArc, UnitPoint, EARTH_RADIUS_METERS},
};

// Preserve every observed fix and densify gaps along the shortest spherical
// arc. Interpolating longitude/latitude independently bends long flights away
// from their route, even when they never enter the bitmap's polar charts.
pub fn process(raw_data: &[Vec<RawGPSPoint>]) -> Option<JourneyVector> {
    const STEP_LENGTH: f64 = 1_000.0;
    let mut track_segments = Vec::new();
    let to_track_point = |data: &RawGPSPoint| TrackPoint {
        latitude: data.point.latitude,
        longitude: data.point.longitude,
    };
    let valid = |data: &RawGPSPoint| {
        data.point.longitude.is_finite()
            && data.point.latitude.is_finite()
            && (-90.0..=90.0).contains(&data.point.latitude)
    };
    let round = |coordinate: f64| (coordinate * 1_000_000.0).round() / 1_000_000.0;

    for source in raw_data {
        let Some(first) = source.first() else {
            continue;
        };
        let mut track_points = vec![to_track_point(first)];
        let mut cumulative_distance = 0.0;
        for pair in source.windows(2) {
            if valid(&pair[0]) && valid(&pair[1]) {
                let arc = GreatCircleArc::new(
                    UnitPoint::from_lng_lat(pair[0].point.longitude, pair[0].point.latitude),
                    UnitPoint::from_lng_lat(pair[1].point.longitude, pair[1].point.latitude),
                );
                let distance = arc.angle * EARTH_RADIUS_METERS;
                let end_distance = cumulative_distance + distance;
                let mut sample_distance =
                    (cumulative_distance / STEP_LENGTH).floor() * STEP_LENGTH + STEP_LENGTH;
                while sample_distance < end_distance {
                    let t = (sample_distance - cumulative_distance) / distance;
                    let (longitude, latitude) = arc.at(t).to_lng_lat();
                    track_points.push(TrackPoint {
                        latitude: round(latitude),
                        longitude: round(longitude),
                    });
                    sample_distance += STEP_LENGTH;
                }
                cumulative_distance = end_distance;
            }
            // Do not normalize, round, simplify, or replace original fixes.
            track_points.push(to_track_point(&pair[1]));
        }
        track_segments.push(TrackSegment { track_points });
    }
    if track_segments.is_empty() {
        None
    } else {
        Some(JourneyVector { track_segments })
    }
}
