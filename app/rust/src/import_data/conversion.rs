use crate::api::import::JourneyInfo;
use crate::flight_track_processor;
use crate::gps_processor::{
    self, GpsPreprocessor, PreprocessedData, ProcessResult, RawData, SegmentGapRule,
};
use crate::journey_date_picker::JourneyDatePicker;
use crate::journey_header::JourneyKind;
use crate::journey_vector::{JourneyVector, TrackPoint};
use chrono::{Local, TimeZone, Utc};

/// `segment_gap_rule_for_preprocessor = None` meaning disable preprocessor
pub fn journey_vector_from_raw_data_with_gps_preprocessor(
    raw_data: &[Vec<RawData>],
    segment_gap_rule_for_preprocessor: Option<SegmentGapRule>,
) -> Option<JourneyVector> {
    let processed_data = raw_data.iter().flat_map(move |x| {
        // we handle each segment separately
        let mut gps_preprocessor =
            segment_gap_rule_for_preprocessor.map(GpsPreprocessor::new_with_rule);

        let mut first = true;
        x.iter().map(move |raw_data| {
            let process_result = match &mut gps_preprocessor {
                Some(preprocessor) => preprocessor.preprocess(raw_data),
                None => {
                    if first {
                        first = false;
                        ProcessResult::NewSegment
                    } else {
                        ProcessResult::Append
                    }
                }
            };

            Ok(PreprocessedData {
                timestamp_sec: raw_data.timestamp_ms.map(|x| x / 1000),
                track_point: TrackPoint {
                    latitude: raw_data.point.latitude,
                    longitude: raw_data.point.longitude,
                },
                process_result,
            })
        })
    });

    gps_processor::build_journey_vector(processed_data, None)
        .expect("Impossible, `preprocessed_data` does not contain error")
}

pub fn journey_vector_from_raw_data_with_flight_track_processor(
    raw_data: &[Vec<RawData>],
) -> Option<JourneyVector> {
    flight_track_processor::process(raw_data)
}

pub fn journey_info_from_raw_vector_data(raw_vector_data: &[Vec<RawData>]) -> JourneyInfo {
    let time_from_raw_data = |raw_data: &RawData| {
        raw_data
            .timestamp_ms
            .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
    };

    let mut journey_date_picker = JourneyDatePicker::new();
    for segment in raw_vector_data {
        for raw_data in segment {
            if let Some(timestamp) = time_from_raw_data(raw_data) {
                journey_date_picker.add_point(
                    timestamp,
                    &TrackPoint {
                        latitude: raw_data.point.latitude,
                        longitude: raw_data.point.longitude,
                    },
                );
            }
        }
    }

    let journey_date = journey_date_picker
        .pick_journey_date()
        .unwrap_or_else(|| Local::now().date_naive());

    JourneyInfo {
        journey_date,
        start_time: journey_date_picker.min_time(),
        end_time: journey_date_picker.max_time(),
        note: None,
        journey_kind: JourneyKind::DefaultKind,
    }
}
