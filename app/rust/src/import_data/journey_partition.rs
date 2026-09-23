use std::collections::BTreeMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};

use crate::gps_processor::RawData;
use crate::import_data::ImportedJourney;
use crate::journey_date_picker::{BoundaryTracker, JourneyDatePicker};
use crate::journey_vector::TrackPoint;

pub type RawDataByDate = BTreeMap<NaiveDate, Vec<Vec<RawData>>>;
pub type SummariesByDate = BTreeMap<NaiveDate, DateSummary>;
pub(crate) type PartitionIndexByDate = BTreeMap<NaiveDate, Vec<SegmentSlice>>;

/// A borrowed slice of one source segment, represented without retaining or
/// cloning any track points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SegmentSlice {
    source_segment: usize,
    start: usize,
    end: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DateSummary {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub point_count: u64,
    pub missing_timestamp_count: u64,
}

impl DateSummary {
    fn add(&mut self, part: &Part) {
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

    fn merge(&mut self, other: &Self) {
        self.point_count += other.point_count;
        self.missing_timestamp_count += other.missing_timestamp_count;
        if let Some(start) = other.start_time {
            self.start_time = Some(self.start_time.map_or(start, |value| value.min(start)));
        }
        if let Some(end) = other.end_time {
            self.end_time = Some(self.end_time.map_or(end, |value| value.max(end)));
        }
    }
}

/// One journey-shaped part of an imported track, before parts with the same
/// chosen date are grouped together.
pub(crate) struct Part {
    pub(crate) date: NaiveDate,
    segments: Vec<SegmentSlice>,
    pub(crate) start: Option<DateTime<Utc>>,
    pub(crate) end: Option<DateTime<Utc>>,
    pub(crate) points: u64,
    pub(crate) untimed: u64,
}

struct PartBuilder {
    segments: Vec<SegmentSlice>,
    date_picker: JourneyDatePicker,
    boundary: BoundaryTracker,
    points: u64,
    untimed: u64,
}

impl PartBuilder {
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

    fn start_segment(&mut self, source_segment: usize, start: usize) {
        self.segments.push(SegmentSlice {
            source_segment,
            start,
            end: start,
        });
    }

    fn should_split_at(&self, timestamp: DateTime<Utc>) -> bool {
        self.has_points() && self.boundary.should_end_at(timestamp.with_timezone(&Local))
    }

    fn push(&mut self, point: &RawData, point_index: usize) {
        self.segments
            .last_mut()
            .expect("a source segment must be started before adding points")
            .end = point_index + 1;
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

    fn finish(self) -> Option<Part> {
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

fn flush(current: &mut PartBuilder, emit: &mut impl FnMut(Part)) {
    let completed = std::mem::replace(current, PartBuilder::new());
    if let Some(part) = completed.finish() {
        emit(part);
    }
}

/// Visits journey-shaped parts without cloning track points. Boundaries follow
/// the recording auto-finalization policy; each completed part gets its date
/// from `JourneyDatePicker`.
fn for_each_part_segments<'a>(
    segments: impl IntoIterator<Item = &'a Vec<RawData>>,
    mut emit: impl FnMut(Part),
) {
    let mut current = PartBuilder::new();

    for (source_segment, segment) in segments
        .into_iter()
        .enumerate()
        .filter(|(_, segment)| !segment.is_empty())
    {
        if let Some(first_time) = segment.iter().find_map(timestamp) {
            if current.should_split_at(first_time) {
                flush(&mut current, &mut emit);
            }
        }
        current.start_segment(source_segment, 0);

        for (point_index, point) in segment.iter().enumerate() {
            if timestamp(point).is_some_and(|time| current.should_split_at(time)) {
                flush(&mut current, &mut emit);
                current.start_segment(source_segment, point_index);
            }
            current.push(point, point_index);
        }
    }

    if current.has_points() {
        flush(&mut current, &mut emit);
    }
}

/// Date index plus per-date summaries from a single segment walk.
pub(crate) struct PartitionByDate {
    pub index: PartitionIndexByDate,
    pub summaries: SummariesByDate,
}

/// Builds the date index and summaries without cloning track points.
pub(crate) fn partition_by_date(raw_data: &[Vec<RawData>]) -> PartitionByDate {
    partition_by_segments(raw_data.iter())
}

fn partition_by_segments<'a>(
    segments: impl IntoIterator<Item = &'a Vec<RawData>>,
) -> PartitionByDate {
    let mut index = PartitionIndexByDate::new();
    let mut summaries = SummariesByDate::new();
    for_each_part_segments(segments, |part| {
        summaries.entry(part.date).or_default().add(&part);
        index.entry(part.date).or_default().extend(part.segments);
    });
    PartitionByDate { index, summaries }
}

/// Builds partitions from explicit MemoLanes journey metadata. This avoids
/// applying timestamp-based splitting to exported points with no point times.
pub(crate) fn partition_by_journey_metadata(groups: &[ImportedJourney]) -> PartitionByDate {
    let mut index = PartitionIndexByDate::new();
    let mut summaries = SummariesByDate::new();
    let mut source_segment = 0usize;

    for group in groups {
        if group.has_memolanes_metadata() {
            let date = group.journey_date.expect("validated MemoLanes date");
            for segment in &group.segments {
                index.entry(date).or_default().push(SegmentSlice {
                    source_segment,
                    start: 0,
                    end: segment.len(),
                });
                let summary = summaries.entry(date).or_default();
                summary.point_count += segment.len() as u64;
                summary.missing_timestamp_count += segment
                    .iter()
                    .filter(|point| point.timestamp_ms.is_none())
                    .count() as u64;
                source_segment += 1;
            }
            let summary = summaries.entry(date).or_default();
            if let Some(start) = group.start {
                summary.start_time =
                    Some(summary.start_time.map_or(start, |value| value.min(start)));
            }
            if let Some(end) = group.end {
                summary.end_time = Some(summary.end_time.map_or(end, |value| value.max(end)));
            }
        } else {
            // Ordinary tracks in a mixed file retain their existing date rules.
            let generic = partition_by_date(&group.segments);
            for (date, slices) in generic.index {
                index
                    .entry(date)
                    .or_default()
                    .extend(slices.into_iter().map(|slice| SegmentSlice {
                        source_segment: slice.source_segment + source_segment,
                        start: slice.start,
                        end: slice.end,
                    }));
                summaries
                    .entry(date)
                    .or_default()
                    .merge(generic.summaries.get(&date).expect("summary exists"));
            }
            source_segment += group.segments.len();
        }
    }
    PartitionByDate { index, summaries }
}

/// Applies ordinary date rules across all groups in a third-party file.
pub(crate) fn partition_generic_groups(groups: &[ImportedJourney]) -> PartitionByDate {
    partition_by_segments(groups.iter().flat_map(|group| group.segments.iter()))
}

/// Materializes only one requested date partition from the source data.
pub(crate) fn materialize_partition(
    raw_data: &[Vec<RawData>],
    partition: &[SegmentSlice],
) -> Vec<Vec<RawData>> {
    partition
        .iter()
        .map(|segment| raw_data[segment.source_segment][segment.start..segment.end].to_vec())
        .collect()
}

/// Resolves global segment numbers directly from the ordered groups.
pub(crate) fn materialize_group_partition(
    groups: &[ImportedJourney],
    partition: &[SegmentSlice],
) -> Vec<Vec<RawData>> {
    let segments = groups
        .iter()
        .flat_map(|group| group.segments.iter())
        .collect::<Vec<_>>();
    partition
        .iter()
        .map(|slice| {
            let segment = segments[slice.source_segment];
            segment[slice.start..slice.end].to_vec()
        })
        .collect()
}

/// Materializes imported points grouped by their chosen journey date.
///
/// This is kept for callers that explicitly need all owned partitions. Import
/// APIs use [`partition_by_date`] and [`materialize_partition`] instead so they
/// never retain a second full copy of the source track.
pub fn group_by_date(raw_data: &[Vec<RawData>]) -> RawDataByDate {
    partition_by_date(raw_data)
        .index
        .into_iter()
        .map(|(date, partition)| (date, materialize_partition(raw_data, &partition)))
        .collect()
}

/// Summarizes the same partition without cloning or retaining track points.
pub fn summarize(raw_data: &[Vec<RawData>]) -> SummariesByDate {
    partition_by_date(raw_data).summaries
}
