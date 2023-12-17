pub struct JourneyVector {
    pub track_segments: Vec<TrackSegment>,
}

pub struct TrackSegment {
    pub track_points: Vec<TrackPoint>,
}

pub struct TrackPoint {
    pub latitude: f64,
    pub longitude: f64,
}
