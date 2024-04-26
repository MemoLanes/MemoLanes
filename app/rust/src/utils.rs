use std::f64::consts::PI;

use chrono::{Datelike, NaiveDate};

// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
pub fn lng_lat_to_tile_x_y(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom);
    let lat_rad = (lat / 180.0) * PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

pub fn tile_x_y_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(PI * (1.0 - (2.0 * y as f64) / n))) * 180.0) / PI;
    (lng, lat)
}

// We could just use num days from ce instead of epoch, but ce is quite far
// away and we use varint for serialization, so epoch can make it a bit more
// efficient.
lazy_static! {
    static ref EPOCH_NUM_OF_DAYS_FROM_CE: i32 = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .num_days_from_ce();
}

pub fn date_to_days_since_epoch(date: NaiveDate) -> i32 {
    date.num_days_from_ce() - *EPOCH_NUM_OF_DAYS_FROM_CE
}

pub fn date_of_days_since_epoch(days: i32) -> Option<NaiveDate> {
    NaiveDate::from_num_days_from_ce_opt(days + *EPOCH_NUM_OF_DAYS_FROM_CE)
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

    use crate::utils::{date_of_days_since_epoch, date_to_days_since_epoch};

    #[test]
    fn days_since_epoch() {
        let check = |y, m, d, expected_days| {
            let date = NaiveDate::from_ymd_opt(y, m, d).unwrap();
            let days = date_to_days_since_epoch(date);
            assert_eq!(days, expected_days);
            assert_eq!(date, date_of_days_since_epoch(days).unwrap());
        };
        check(1970, 1, 1, 0);
        check(2024, 2, 29, 19782);
        check(1938, 8, 23, -11454);
    }

    #[test]
    fn naive_date_is_local_date() {
        let utc = Utc.with_ymd_and_hms(2024, 3, 31, 23, 0, 0).unwrap();
        assert_eq!(utc.to_rfc3339(), "2024-03-31T23:00:00+00:00");
        let plus8 = utc.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
        assert_eq!(plus8.to_rfc3339(), "2024-04-01T07:00:00+08:00");
        assert_eq!(utc.date_naive().to_string(), "2024-03-31");
        assert_eq!(plus8.date_naive().to_string(), "2024-04-01");
    }
}
