use anyhow::Result;
use chrono::{DateTime, Local};

use crate::gps::{ExtendedRawGPSPoint, Point};
use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::main_db::Txn;

pub(crate) struct RecordedLocation {
    pub process_result: ProcessResult,
    pub previous_point: Option<Point>,
    pub journey_finalized: bool,
}

impl RecordedLocation {
    fn ignored() -> Self {
        Self {
            process_result: ProcessResult::Ignore,
            previous_point: None,
            journey_finalized: false,
        }
    }
}

/// Applies one live or delayed location on the provider's timeline. The caller
/// commits the updated preprocessor only after this database transaction succeeds.
pub(crate) fn record_location(
    txn: &mut Txn,
    preprocessor: &mut GpsPreprocessor,
    data: &ExtendedRawGPSPoint,
    retain_raw_data: bool,
) -> Result<RecordedLocation> {
    let location = &data.raw_gps_point;
    let Some(timestamp_ms) = location.timestamp_ms else {
        return Ok(RecordedLocation::ignored());
    };
    let Some(time) = DateTime::from_timestamp_millis(timestamp_ms) else {
        return Ok(RecordedLocation::ignored());
    };
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

    let raw_data = retain_raw_data.then_some(data);
    if !preprocessor.accept_location(location) {
        txn.record(location, ProcessResult::Ignore, raw_data)?;
        return Ok(RecordedLocation::ignored());
    }

    // A batch can contain several journeys. Close the previous one before
    // inserting the first point after a qualifying gap, even during replay.
    let journey_finalized =
        txn.try_auto_finalize_journey_at(time.with_timezone(&Local), retain_raw_data)?;
    if journey_finalized {
        *preprocessor = GpsPreprocessor::new();
        // Preserve the accepted point in the new detector's history too.
        preprocessor.accept_location(location);
    }
    let previous_point = preprocessor.last_kept_point();
    let process_result = preprocessor.preprocess_accepted_location(location);
    txn.record(location, process_result, raw_data)?;
    txn.set_last_recording_timestamp_ms(timestamp_ms)?;

    Ok(RecordedLocation {
        process_result,
        previous_point,
        journey_finalized,
    })
}

#[cfg(test)]
#[path = "../tests/unit/recording_boundaries.rs"]
mod boundary_tests;
