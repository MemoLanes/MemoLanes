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

pub(crate) fn record_locations(
    txn: &mut Txn,
    preprocessor: &mut GpsPreprocessor,
    updates: &[ExtendedRawGPSPoint],
    retain_raw_data: bool,
) -> Result<Vec<RecordedLocation>> {
    let previous = txn.last_recording_timestamp_ms()?;
    let mut last = previous;
    let recorded = updates
        .iter()
        .map(|update| record_location(txn, preprocessor, update, retain_raw_data, &mut last))
        .collect::<Result<Vec<_>>>()?;
    // After finalizing, the new timeline can reach the same timestamp again.
    if let Some(timestamp_ms) = last.filter(|timestamp| {
        Some(*timestamp) != previous || recorded.iter().any(|fix| fix.journey_finalized)
    }) {
        txn.set_last_recording_timestamp_ms(timestamp_ms)?;
    }
    Ok(recorded)
}

fn record_location(
    txn: &mut Txn,
    preprocessor: &mut GpsPreprocessor,
    data: &ExtendedRawGPSPoint,
    retain_raw_data: bool,
    last: &mut Option<i64>,
) -> Result<RecordedLocation> {
    let location = &data.raw_gps_point;
    let Some(timestamp_ms) = location.timestamp_ms else {
        return Ok(RecordedLocation::ignored());
    };
    let Some(time) = DateTime::from_timestamp_millis(timestamp_ms) else {
        return Ok(RecordedLocation::ignored());
    };
    let received_at = data.received_timestamp_ms;
    if timestamp_ms > received_at
        || !(-90.0..=90.0).contains(&location.point.latitude)
        || !(-180.0..=180.0).contains(&location.point.longitude)
        || location
            .accuracy
            .is_some_and(|accuracy| !accuracy.is_finite() || accuracy < 0.0)
    {
        return Ok(RecordedLocation::ignored());
    }

    // Only record fixes captured by their original receipt time, so an
    // earlier receipt than the last accepted fix indicates a clock rewind.
    let clock_rolled_back = last.is_some_and(|last| last > received_at);
    if !clock_rolled_back && last.is_some_and(|last| timestamp_ms <= last) {
        return Ok(RecordedLocation::ignored());
    }

    let raw_data = retain_raw_data.then_some(data);
    let mut reset_preprocessor = clock_rolled_back.then(GpsPreprocessor::new);
    if !reset_preprocessor
        .as_mut()
        .unwrap_or(preprocessor)
        .accept_location(location)
    {
        // Wait for a usable fix before recording on the new timeline.
        if !clock_rolled_back {
            txn.record(location, ProcessResult::Ignore, raw_data)?;
        }
        return Ok(RecordedLocation::ignored());
    }

    // A batch can contain several journeys. Close the previous one before
    // inserting the first point after a qualifying gap.
    let journey_finalized = if let Some(reset) = reset_preprocessor {
        *preprocessor = reset;
        txn.finalize_ongoing_journey(retain_raw_data)?
    } else {
        txn.try_auto_finalize_journey_at(time.with_timezone(&Local), retain_raw_data)?
    };
    if journey_finalized && !clock_rolled_back {
        *preprocessor = GpsPreprocessor::new();
        // Preserve the accepted point in the new detector's history too.
        preprocessor.accept_location(location);
    }
    let previous_point = preprocessor.last_kept_point();
    let process_result = preprocessor.preprocess_accepted_location(location);
    txn.record(location, process_result, raw_data)?;
    *last = Some(timestamp_ms);

    Ok(RecordedLocation {
        process_result,
        previous_point,
        journey_finalized,
    })
}

#[cfg(test)]
#[path = "../tests/unit/recording_boundaries.rs"]
mod boundary_tests;
