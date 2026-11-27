use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::gps::{ExtendedRawGPSPoint, Point};
use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::main_db::Txn;

// Allow small clock differences without rejecting fixes delivered from the cache.
const MAX_FUTURE_OFFSET_MS: i64 = 30_000;

pub(crate) struct RecordedLocation {
    pub process_result: ProcessResult,
    pub previous_point: Option<Point>,
}

impl RecordedLocation {
    fn ignored() -> Self {
        Self {
            process_result: ProcessResult::Ignore,
            previous_point: None,
        }
    }
}

pub(crate) fn eligible_capture_time(
    txn: &Txn,
    data: &ExtendedRawGPSPoint,
) -> Result<Option<DateTime<Utc>>> {
    let location = &data.raw_gps_point;
    let Some(capture_time) = location
        .timestamp_ms
        .and_then(DateTime::from_timestamp_millis)
    else {
        return Ok(None);
    };
    let timestamp_ms = capture_time.timestamp_millis();
    let future_offset_ms = timestamp_ms.saturating_sub(data.received_timestamp_ms);
    if future_offset_ms > MAX_FUTURE_OFFSET_MS
        || !location.point.latitude.is_finite()
        || !location.point.longitude.is_finite()
        || !(-90.0..=90.0).contains(&location.point.latitude)
        || !(-180.0..=180.0).contains(&location.point.longitude)
        || location
            .accuracy
            .is_some_and(|accuracy| !accuracy.is_finite() || accuracy < 0.0)
    {
        return Ok(None);
    }
    // Native providers can deliver fixes out of order or replay a committed fix.
    if txn
        .last_recording_timestamp_ms()?
        .is_some_and(|last| timestamp_ms <= last)
    {
        return Ok(None);
    }
    Ok(Some(capture_time))
}

// The caller commits the preprocessor only after the database transaction succeeds.
pub(crate) fn record_location(
    txn: &mut Txn,
    preprocessor: &mut GpsPreprocessor,
    data: &ExtendedRawGPSPoint,
    record_raw_data: bool,
) -> Result<RecordedLocation> {
    let Some(capture_time) = eligible_capture_time(txn, data)? else {
        return Ok(RecordedLocation::ignored());
    };
    let location = &data.raw_gps_point;
    let raw_data = record_raw_data.then_some(data);
    if !preprocessor.accept_location(location) {
        // shortcut: bad-fix diagnostics may repeat after a failed acknowledgement; deduplicate if this becomes costly.
        txn.record(location, ProcessResult::Ignore, raw_data)?;
        return Ok(RecordedLocation::ignored());
    }

    let previous_point = preprocessor.last_kept_point();
    let process_result = preprocessor.preprocess_accepted_location(location);
    txn.record(location, process_result, raw_data)?;
    txn.set_last_recording_timestamp_ms(capture_time.timestamp_millis())?;
    Ok(RecordedLocation {
        process_result,
        previous_point,
    })
}

#[cfg(test)]
#[path = "../tests/unit/recording.rs"]
mod tests;
