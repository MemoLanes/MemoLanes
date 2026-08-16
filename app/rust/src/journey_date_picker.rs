use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Timelike, Utc};

use crate::{gps_processor::Point, journey_vector::TrackPoint};

/// Returns the idle gap required to end a journey at `now`.
pub(crate) fn min_gap<Tz: TimeZone>(
    start: DateTime<Utc>,
    now: DateTime<Tz>,
    duration_hours: i64,
) -> i64 {
    if duration_hours >= 48 {
        0 // Finalize very long recordings immediately.
    } else if duration_hours >= 24 {
        2
    } else {
        // On the starting date, avoid ending a journey unless there is a huge
        // gap. After crossing midnight, shorter gaps are enough, while the
        // first few hours retain a 20-minute grace period.
        if start.with_timezone(&now.timezone()).date_naive() == now.date_naive() {
            6 * 60
        } else if now.hour() <= 4 || duration_hours <= 8 {
            20
        } else {
            5
        }
    }
}

/// Tracks enough history to apply `min_gap` to a stream of timestamps.
#[derive(Default)]
pub(crate) struct BoundaryTracker {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl BoundaryTracker {
    pub(crate) fn should_end_at<Tz: TimeZone>(&self, now: DateTime<Tz>) -> bool {
        let (Some(start), Some(end)) = (self.start, self.end) else {
            return false;
        };
        let duration_hours = (now.timestamp() - start.timestamp()) / 60 / 60;
        let gap_mins = (now.timestamp() - end.timestamp()).max(0) / 60;
        gap_mins >= min_gap(start, now, duration_hours)
    }

    pub(crate) fn observe(&mut self, timestamp: DateTime<Utc>) {
        self.start.get_or_insert(timestamp);
        self.end = Some(timestamp);
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
