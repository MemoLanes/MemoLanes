use anyhow::Result;
use chrono::DateTime;

use crate::gps::{ExtendedRawGPSPoint, Point};
use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::main_db::Txn;

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

// The caller commits the preprocessor only after the database transaction succeeds.
pub(crate) fn record_location(
    txn: &mut Txn,
    preprocessor: &mut GpsPreprocessor,
    data: &ExtendedRawGPSPoint,
    record_raw_data: bool,
) -> Result<RecordedLocation> {
    let location = &data.raw_gps_point;
    let Some(timestamp_ms) = location
        .timestamp_ms
        .filter(|timestamp| DateTime::from_timestamp_millis(*timestamp).is_some())
    else {
        return Ok(RecordedLocation::ignored());
    };
    // Native providers can deliver fixes out of order (observed on Android).
    // Check persisted capture time before changing preprocessing state.
    if txn
        .last_recording_timestamp_ms()?
        .is_some_and(|last| timestamp_ms <= last)
        || !location.point.latitude.is_finite()
        || !location.point.longitude.is_finite()
        || !(-90.0..=90.0).contains(&location.point.latitude)
        || !(-180.0..=180.0).contains(&location.point.longitude)
        || location
            .accuracy
            .is_some_and(|accuracy| !accuracy.is_finite() || accuracy < 0.0)
    {
        return Ok(RecordedLocation::ignored());
    }
    let raw_data = record_raw_data.then_some(data);
    if !preprocessor.accept_location(location) {
        // shortcut: bad-fix diagnostics may repeat after a failed acknowledgement; deduplicate if this becomes costly.
        txn.record(location, ProcessResult::Ignore, raw_data)?;
        return Ok(RecordedLocation::ignored());
    }

    let previous_point = preprocessor.last_kept_point();
    let process_result = preprocessor.preprocess_accepted_location(location);
    txn.record(location, process_result, raw_data)?;
    txn.set_last_recording_timestamp_ms(timestamp_ms)?;
    Ok(RecordedLocation {
        process_result,
        previous_point,
    })
}

#[cfg(test)]
#[path = "../tests/unit/recording.rs"]
mod tests;
