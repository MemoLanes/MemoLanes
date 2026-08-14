use std::collections::BTreeMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};

use crate::gps_processor::RawData;

pub type RawVectorDataByDate = BTreeMap<NaiveDate, Vec<Vec<RawData>>>;
pub type RawVectorDataSummaryByDate = BTreeMap<NaiveDate, RawVectorDataDateSummary>;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RawVectorDataDateSummary {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub point_count: u64,
    pub missing_timestamp_count: u64,
}

impl RawVectorDataDateSummary {
    fn add_point(&mut self, point: &RawData) {
        self.point_count += 1;
        if point.timestamp_ms.is_none() {
            self.missing_timestamp_count += 1;
        }

        let Some(timestamp) = point
            .timestamp_ms
            .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
        else {
            return;
        };
        self.start_time = Some(
            self.start_time
                .map_or(timestamp, |start_time| start_time.min(timestamp)),
        );
        self.end_time = Some(
            self.end_time
                .map_or(timestamp, |end_time| end_time.max(timestamp)),
        );
    }

    fn add_pending_untimed(&mut self, point_count: u64, missing_timestamp_count: u64) {
        self.point_count += point_count;
        self.missing_timestamp_count += missing_timestamp_count;
    }
}

fn local_date_of_timestamp(timestamp_ms: i64) -> Option<NaiveDate> {
    Utc.timestamp_millis_opt(timestamp_ms)
        .single()
        .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
}

/// Splits raw vector data into local calendar days while preserving source
/// segment boundaries. Points without a timestamp inherit the nearest known
/// date in their segment. Completely untimed leading segments are attached to
/// the first dated segment; later untimed segments inherit the preceding date.
pub fn split_raw_vector_data_by_local_date(raw_data: &[Vec<RawData>]) -> RawVectorDataByDate {
    split_raw_vector_data_by(raw_data, local_date_of_timestamp)
}

/// Summarizes the local-date split without cloning or retaining any points.
pub fn summarize_raw_vector_data_by_local_date(
    raw_data: &[Vec<RawData>],
) -> RawVectorDataSummaryByDate {
    let mut result = BTreeMap::new();
    let mut previous_date = None;
    let mut pending_point_count = 0;
    let mut pending_missing_timestamp_count = 0;

    for source_segment in raw_data.iter().filter(|segment| !segment.is_empty()) {
        let first_date = source_segment
            .iter()
            .filter_map(|point| point.timestamp_ms.and_then(local_date_of_timestamp))
            .next();

        let Some(mut current_date) = first_date.or(previous_date) else {
            pending_point_count += source_segment.len() as u64;
            pending_missing_timestamp_count += source_segment
                .iter()
                .filter(|point| point.timestamp_ms.is_none())
                .count() as u64;
            continue;
        };

        if let Some(first_date) = first_date {
            result
                .entry(first_date)
                .or_insert_with(RawVectorDataDateSummary::default)
                .add_pending_untimed(pending_point_count, pending_missing_timestamp_count);
            pending_point_count = 0;
            pending_missing_timestamp_count = 0;
        }

        for point in source_segment {
            let point_date = point
                .timestamp_ms
                .and_then(local_date_of_timestamp)
                .unwrap_or(current_date);
            result
                .entry(point_date)
                .or_insert_with(RawVectorDataDateSummary::default)
                .add_point(point);
            current_date = point_date;
        }
        previous_date = Some(current_date);
    }

    result
}

fn split_raw_vector_data_by(
    raw_data: &[Vec<RawData>],
    date_of_timestamp: impl Fn(i64) -> Option<NaiveDate>,
) -> RawVectorDataByDate {
    let mut result = BTreeMap::new();
    let mut previous_date = None;
    let mut pending_untimed_segments: Vec<Vec<RawData>> = Vec::new();

    for source_segment in raw_data.iter().filter(|segment| !segment.is_empty()) {
        let first_date = source_segment
            .iter()
            .filter_map(|point| point.timestamp_ms.and_then(&date_of_timestamp))
            .next();

        let Some(mut current_date) = first_date.or(previous_date) else {
            pending_untimed_segments.push(source_segment.clone());
            continue;
        };

        if let Some(first_date) = first_date {
            for segment in pending_untimed_segments.drain(..) {
                result
                    .entry(first_date)
                    .or_insert_with(Vec::new)
                    .push(segment);
            }
        }

        let mut current_segment = Vec::new();
        for point in source_segment {
            let point_date = point
                .timestamp_ms
                .and_then(&date_of_timestamp)
                .unwrap_or(current_date);
            if point_date != current_date && !current_segment.is_empty() {
                result
                    .entry(current_date)
                    .or_insert_with(Vec::new)
                    .push(current_segment);
                current_segment = Vec::new();
            }
            current_date = point_date;
            current_segment.push(point.clone());
        }
        if !current_segment.is_empty() {
            result
                .entry(current_date)
                .or_insert_with(Vec::new)
                .push(current_segment);
        }
        previous_date = Some(current_date);
    }

    // This only happens when the entire file has no timestamps. Returning an
    // empty map signals that date splitting is unavailable and the existing
    // single-journey import flow should be used.
    result
}
