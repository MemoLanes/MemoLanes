use std::collections::BTreeMap;

use chrono::{Local, NaiveDate, TimeZone, Utc};

use crate::gps_processor::RawData;

pub type RawVectorDataByDate = BTreeMap<NaiveDate, Vec<Vec<RawData>>>;

/// Splits raw vector data into local calendar days while preserving source
/// segment boundaries. Points without a timestamp inherit the nearest known
/// date in their segment. Completely untimed leading segments are attached to
/// the first dated segment; later untimed segments inherit the preceding date.
pub fn split_raw_vector_data_by_local_date(raw_data: &[Vec<RawData>]) -> RawVectorDataByDate {
    split_raw_vector_data_by(raw_data, |timestamp_ms| {
        Utc.timestamp_millis_opt(timestamp_ms)
            .single()
            .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
    })
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
