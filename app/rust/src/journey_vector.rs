use flutter_rust_bridge::frb;

#[derive(Debug, PartialEq)]
#[frb(opaque)]
pub struct JourneyVector {
    pub track_segments: Vec<TrackSegment>,
}

#[derive(Debug, PartialEq)]
pub struct TrackSegment {
    pub track_points: Vec<TrackPoint>,
}

#[derive(Debug, PartialEq)]
pub struct TrackPoint {
    pub latitude: f64,
    pub longitude: f64,
}
