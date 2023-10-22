// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn lng_lat_to_tile_xy(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom);
    let lat_rad = (lat / 180.0) * std::f64::consts::PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

pub fn tile_xy_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(
        std::f64::consts::PI * (1.0 - (2.0 * y as f64) / n),
    )) * 180.0)
        / std::f64::consts::PI;
    (lng, lat)
}
