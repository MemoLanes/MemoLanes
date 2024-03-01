use anyhow::Result;
use geo_types::Point;
use crate::journey_vector::JourneyVector;
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};

pub fn journey_vector_to_gpx(journey_vector: &JourneyVector) -> Result<Gpx> {
    let mut segments = Vec::new();
    // Add track point
    for track_segment  in &journey_vector.track_segments{
        let mut points = Vec::new();
        track_segment.track_points.iter().for_each(|point|{
            points.push(Waypoint::new(Point::new(point.latitude, point.longitude)));
        });
        segments.push(TrackSegment {
            points: points
        });
    }
    let track = Track {
        name: Some("Track 1".to_string()),
        comment: None,
        description: None,
        source: None,
        links: vec![],
        type_: None,
        number: None,
        segments: segments,
    };
    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        creator: None,
        metadata: None,
        waypoints: vec![],
        tracks: vec![track],
        routes: vec![],
    };
    Ok(gpx)
    
}