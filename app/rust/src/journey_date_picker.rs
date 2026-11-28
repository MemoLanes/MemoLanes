use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Timelike, Utc};

use crate::{gps::Point, journey_vector::TrackPoint};

fn should_end_journey(start_ms: i64, end_ms: i64, now_ms: i64) -> bool {
    const MINUTE_MS: i64 = 60_000;
    let duration_hours = (now_ms.div_euclid(1000) - start_ms.div_euclid(1000)) / 3600;
    if duration_hours >= 48 {
        return true;
    }
    let gap_ms = now_ms.saturating_sub(end_ms);
    if duration_hours >= 24 {
        return gap_ms >= 2 * MINUTE_MS;
    }
    if gap_ms < 5 * MINUTE_MS {
        return false;
    }

    let (Some(start), Some(now)) = (
        Local.timestamp_millis_opt(start_ms).single(),
        Local.timestamp_millis_opt(now_ms).single(),
    ) else {
        return false;
    };
    let required_minutes = if start.date_naive() == now.date_naive() {
        6 * 60
    } else if now.hour() <= 4 || duration_hours <= 8 {
        20
    } else {
        5
    };
    gap_ms >= required_minutes * MINUTE_MS
}

#[derive(Default)]
pub(crate) struct BoundaryTracker {
    start_ms: Option<i64>,
    end_ms: Option<i64>,
}

impl BoundaryTracker {
    pub(crate) fn from_range(range_ms: Option<(i64, i64)>) -> Self {
        let (start_ms, end_ms) = range_ms.unzip();
        Self { start_ms, end_ms }
    }

    pub(crate) fn has_timestamps(&self) -> bool {
        self.start_ms.is_some()
    }

    pub(crate) fn should_end_at(&self, now_ms: i64) -> bool {
        let (Some(start_ms), Some(end_ms)) = (self.start_ms, self.end_ms) else {
            return false;
        };
        should_end_journey(start_ms, end_ms, now_ms)
    }

    pub(crate) fn observe(&mut self, timestamp_ms: i64) {
        self.start_ms.get_or_insert(timestamp_ms);
        self.end_ms = self.end_ms.max(Some(timestamp_ms));
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn late_samples_do_not_move_the_boundary_backwards() {
        let start_ms = Local
            .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_millis();
        let at_hour = |hour: i64| start_ms + hour * 3_600_000;
        let mut tracker = BoundaryTracker::default();
        for hour in [0, 5, 1] {
            tracker.observe(at_hour(hour));
        }
        assert!(!tracker.should_end_at(at_hour(7)));
        assert!(tracker.should_end_at(at_hour(11)));
    }

    #[test]
    fn five_minute_gap_after_midnight_starts_qualifying_at_local_five_am() {
        let local_ms = |day, hour, minute| {
            Local
                .with_ymd_and_hms(2025, 1, day, hour, minute, 0)
                .unwrap()
                .timestamp_millis()
        };
        let start = local_ms(1, 18, 0);
        assert!(!should_end_journey(
            start,
            local_ms(2, 4, 54),
            local_ms(2, 4, 59)
        ));
        assert!(should_end_journey(
            start,
            local_ms(2, 4, 55),
            local_ms(2, 5, 0)
        ));
    }
}

// TODO: I think using `chrono::Local` might be problematic. I think on mobile
// devices, the timezone might change as the user travels. We should probably
// use the flutter side to get the current timezone and pass it in here.

// Tools for picking the journey date based on a series of GPS data with timestamp.
// We track the two furthest points of each day and use the distance between
// to measure how "big" each day is. The we pick the latest day after filtering out
// days that are too small.
pub struct JourneyDatePicker {
    furthest_point_tracker_per_day: HashMap<NaiveDate, FurthestPointTracker>,
    min_time: Option<DateTime<Utc>>,
    max_time: Option<DateTime<Utc>>,
}

impl JourneyDatePicker {
    pub fn new() -> Self {
        JourneyDatePicker {
            furthest_point_tracker_per_day: HashMap::new(),
            min_time: None,
            max_time: None,
        }
    }

    pub fn add_point(&mut self, time: DateTime<Utc>, point: &TrackPoint) {
        let date = time.with_timezone(&Local).date_naive();
        self.furthest_point_tracker_per_day
            .entry(date)
            .and_modify(|x| x.update(point))
            .or_insert(FurthestPointTracker::new(point));
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));
        self.max_time = Some(self.max_time.map_or(time, |t| t.max(time)));
    }

    pub fn pick_journey_date(&self) -> Option<NaiveDate> {
        let mut max_distance: f64 = 0.;
        let mut distance_per_date = Vec::with_capacity(self.furthest_point_tracker_per_day.len());
        for (date, tracker) in self.furthest_point_tracker_per_day.iter() {
            let distance = tracker.distance_in_m();
            max_distance = max_distance.max(distance);
            distance_per_date.push((*date, distance));
        }
        let meaningful_dates = distance_per_date
            .iter()
            .filter(|(_, distance)| *distance >= max_distance * 0.4)
            .map(|(date, _)| *date);

        let journey_date = meaningful_dates.max(); // break ties by picking the latest date

        info!("Picked journey date: {journey_date:?}. distance_per_date={distance_per_date:?}");

        journey_date
    }

    pub fn min_time(&self) -> Option<DateTime<Utc>> {
        self.min_time
    }

    pub fn max_time(&self) -> Option<DateTime<Utc>> {
        self.max_time
    }
}

struct FurthestPointTracker {
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
}

impl FurthestPointTracker {
    fn new(point: &TrackPoint) -> Self {
        FurthestPointTracker {
            lat_min: point.latitude,
            lat_max: point.latitude,
            lon_min: point.longitude,
            lon_max: point.longitude,
        }
    }

    fn update(&mut self, point: &TrackPoint) {
        // TODO: This doesn't work super well for points around antimeridian.
        // But should be good enough for most use cases.
        self.lat_min = self.lat_min.min(point.latitude);
        self.lat_max = self.lat_max.max(point.latitude);
        self.lon_min = self.lon_min.min(point.longitude);
        self.lon_max = self.lon_max.max(point.longitude);
    }

    fn distance_in_m(&self) -> f64 {
        let point1 = Point {
            latitude: self.lat_min,
            longitude: self.lon_min,
        };
        let point2 = Point {
            latitude: self.lat_max,
            longitude: self.lon_max,
        };
        point1.haversine_distance(&point2)
    }
}
