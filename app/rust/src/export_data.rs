use crate::journey_vector::JourneyVector;
use anyhow::Result;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
use kml::{types::Placemark, Kml};

pub fn journey_vector_to_gpx(journey_vector: &JourneyVector) -> Result<Gpx> {
    let mut segments = Vec::new();
    // Add track point
    for track_segment in &journey_vector.track_segments {
        let mut points = Vec::new();
        track_segment.track_points.iter().for_each(|point| {
            points.push(Waypoint::new(Point::new(point.latitude, point.longitude)));
        });
        segments.push(TrackSegment { points: points });
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

pub fn journey_vector_to_kml(journey_vector: &JourneyVector) -> Result<Kml> {
    let mut buf = Vec::new();
    for track_segment in &journey_vector.track_segments {
        let mut points = Vec::new();
        track_segment.track_points.iter().for_each(|point| {
            points.push(Point::from(Point::new(point.latitude, point.longitude)));
        });
    }

    let placemark = Placemark::new(
        // 设置Placemark的名称
        "Example Placemark".to_string(),
        // 设置Placemark的描述
        Some("This is an example Placemark".to_string()),
        // 设置Placemark的地理位置（这里仅为示例，需要实际坐标）
        Some(kml::Geometry::new_point(kml::Point::new(
            kml::Coordinate::new(0.0, 0.0, Some(0.0)),
        ))),
    );

    // 将Placemark添加到KML文档中
    kml.add_feature(placemark);

    let kml = Point::from(geo_point);
}

fn main() -> std::io::Result<()> {
    let mut kml = Kml::new();

    // 创建一个文件用于输出KML
    let mut file = File::create("output.kml")?;

    // 将KML文档写入文件
    file.write_all(kml.to_string().as_bytes())?;

    Ok(())
}
