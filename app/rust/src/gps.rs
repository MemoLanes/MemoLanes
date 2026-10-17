//! GPS coordinates and samples shared by processing, import, and storage.

// TODO: This is the same as `TrackPoint`, we should unify them.
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

    pub fn to_cartesian(&self) -> (f64, f64, f64) {
        let lon_rad = Point::to_radians(self.longitude);
        let lat_rad = Point::to_radians(self.latitude);
        let x = lat_rad.cos() * lon_rad.cos();
        let y = lat_rad.cos() * lon_rad.sin();
        let z = lat_rad.sin();
        (x, y, z)
    }

    pub fn to_geographic(x: f64, y: f64, z: f64) -> Point {
        let lon = Point::to_degrees(y.atan2(x));
        let lat = Point::to_degrees(z.atan2((x * x + y * y).sqrt()));
        Point {
            latitude: lat,
            longitude: Point::normalize_longitude(lon),
        }
    }

    fn to_radians(deg: f64) -> f64 {
        use std::f64::consts::PI;
        deg * PI / 180.0
    }
    fn to_degrees(rad: f64) -> f64 {
        use std::f64::consts::PI;
        rad * 180.0 / PI
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawGPSPoint {
    pub point: Point,
    pub timestamp_ms: Option<i64>,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtendedRawGPSPoint {
    pub raw_gps_point: RawGPSPoint,
    pub received_timestamp_ms: i64,
}
