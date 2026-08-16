use std::collections::BTreeMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};

use crate::gps_processor::RawData;
use crate::journey_date_picker::{BoundaryTracker, JourneyDatePicker};
use crate::journey_vector::TrackPoint;

pub type RawDataByDate = BTreeMap<NaiveDate, Vec<Vec<RawData>>>;
pub type SummariesByDate = BTreeMap<NaiveDate, DateSummary>;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DateSummary {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub point_count: u64,
    pub missing_timestamp_count: u64,
}

impl DateSummary {
    fn add(&mut self, part: &Part<'_>) {
        self.point_count += part.points;
        self.missing_timestamp_count += part.untimed;
        if let Some(start_time) = part.start {
            self.start_time = Some(
                self.start_time
                    .map_or(start_time, |current| current.min(start_time)),
            );
        }
        if let Some(end_time) = part.end {
            self.end_time = Some(
                self.end_time
                    .map_or(end_time, |current| current.max(end_time)),
            );
        }
    }
}

/// One journey-shaped part of an imported track, before parts with the same
/// chosen date are grouped together.
pub(crate) struct Part<'a> {
    pub(crate) date: NaiveDate,
    pub(crate) segments: Vec<Vec<&'a RawData>>,
    pub(crate) start: Option<DateTime<Utc>>,
    pub(crate) end: Option<DateTime<Utc>>,
    pub(crate) points: u64,
    pub(crate) untimed: u64,
}

struct PartBuilder<'a> {
    segments: Vec<Vec<&'a RawData>>,
    date_picker: JourneyDatePicker,
    boundary: BoundaryTracker,
    points: u64,
    untimed: u64,
}

impl<'a> PartBuilder<'a> {
    fn new() -> Self {
        Self {
            segments: Vec::new(),
            date_picker: JourneyDatePicker::new(),
            boundary: BoundaryTracker::default(),
            points: 0,
            untimed: 0,
        }
    }

    fn has_points(&self) -> bool {
        self.points != 0
    }

    fn start_segment(&mut self) {
        self.segments.push(Vec::new());
    }

    fn should_split_at(&self, timestamp: DateTime<Utc>) -> bool {
        self.has_points() && self.boundary.should_end_at(timestamp.with_timezone(&Local))
    }

    fn push(&mut self, point: &'a RawData) {
        self.segments
            .last_mut()
            .expect("a source segment must be started before adding points")
            .push(point);
        self.points += 1;

        let Some(timestamp) = timestamp(point) else {
            self.untimed += 1;
            return;
        };
        self.boundary.observe(timestamp);
        self.date_picker.add_point(
            timestamp,
            &TrackPoint {
                latitude: point.point.latitude,
                longitude: point.point.longitude,
            },
        );
    }

    fn finish(self) -> Option<Part<'a>> {
        let date = self.date_picker.pick_journey_date()?;
        Some(Part {
            date,
            segments: self.segments,
            start: self.date_picker.min_time(),
            end: self.date_picker.max_time(),
            points: self.points,
            untimed: self.untimed,
        })
    }
}

fn timestamp(point: &RawData) -> Option<DateTime<Utc>> {
    point
        .timestamp_ms
        .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
}

fn flush<'a>(current: &mut PartBuilder<'a>, emit: &mut impl FnMut(Part<'a>)) {
    let completed = std::mem::replace(current, PartBuilder::new());
    if let Some(part) = completed.finish() {
        emit(part);
    }
}

/// Visits journey-shaped parts without cloning track points. Boundaries follow
/// the recording auto-finalization policy; each completed part gets its date
/// from `JourneyDatePicker`.
pub(crate) fn for_each_part<'a>(raw_data: &'a [Vec<RawData>], mut emit: impl FnMut(Part<'a>)) {
    let mut current = PartBuilder::new();

    for segment in raw_data.iter().filter(|segment| !segment.is_empty()) {
        if let Some(first_time) = segment.iter().find_map(timestamp) {
            if current.should_split_at(first_time) {
                flush(&mut current, &mut emit);
            }
        }
        current.start_segment();

        for point in segment {
            if timestamp(point).is_some_and(|time| current.should_split_at(time)) {
                flush(&mut current, &mut emit);
                current.start_segment();
            }
            current.push(point);
        }
    }

    if current.has_points() {
        flush(&mut current, &mut emit);
    }
}

/// Materializes imported points grouped by their chosen journey date.
pub fn group_by_date(raw_data: &[Vec<RawData>]) -> RawDataByDate {
    let mut result = BTreeMap::new();
    for_each_part(raw_data, |part| {
        let segments = result.entry(part.date).or_insert_with(Vec::new);
        segments.extend(
            part.segments
                .into_iter()
                .map(|segment| segment.into_iter().cloned().collect::<Vec<RawData>>()),
        );
    });
    result
}

/// Summarizes the same partition without cloning or retaining track points.
pub fn summarize(raw_data: &[Vec<RawData>]) -> SummariesByDate {
    let mut result = BTreeMap::new();
    for_each_part(raw_data, |part| {
        result
            .entry(part.date)
            .or_insert_with(DateSummary::default)
            .add(&part);
    });
    result
}
