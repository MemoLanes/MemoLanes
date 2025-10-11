use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, Utc};

use crate::{gps_processor::Point, journey_vector::TrackPoint};

// Tools for picking the journey date based on a series of GPS data with timestamp.
// We track the two furthest points of each day and use the distance between
// to measure how "big" each day is. The we pick the latest day after filtering out
// days that are less than half of the biggest day.
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
            .filter(|(_, distance)| *distance >= max_distance / 2.)
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
