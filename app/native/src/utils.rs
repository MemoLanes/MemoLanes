use std::f64::consts::PI;

// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn lng_lat_to_tile_xy(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let mul = (1 << zoom) as f64;
    let x = (lng + 180.0) / 360.0 * mul;
    let y = (PI - (lat * PI / 180.0).tan().asinh()) * mul / (2.0 * PI);
    (x as i32, y as i32)
}

pub fn tile_xy_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let mul = (1 << zoom) as f64;
    let lng = (x as f64 / mul) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(PI * (1.0 - (2.0 * y as f64) / mul))) * 180.0) / PI;
    (lng, lat)
}
