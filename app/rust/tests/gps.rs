use memolanes_core::gps::Point;

fn point(latitude: f64, longitude: f64) -> Point {
    Point {
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

    assert_eq!(
        point(0.0, 0.1).haversine_distance(&point(0.0, -0.1)) as i32,
        22238
    );

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
