use crate::flight_track_processor;
use crate::gps::RawGPSPoint;
use crate::gps_processor::{
    self, GpsPreprocessor, PreprocessedData, ProcessResult, SegmentGapRule,
};
use crate::import_data::ImportPreprocessor;
use crate::journey_data::JourneyData;
use crate::journey_date_picker::JourneyDatePicker;
use crate::journey_header::{JourneyInfo, JourneyKind};
use crate::journey_vector::{JourneyVector, TrackPoint};
use chrono::{Local, TimeZone, Utc};

/// `segment_gap_rule_for_preprocessor = None` meaning disable preprocessor
pub fn journey_vector_from_raw_data_with_gps_preprocessor<'a, Segment>(
    raw_data: impl IntoIterator<Item = &'a Segment>,
    segment_gap_rule_for_preprocessor: Option<SegmentGapRule>,
) -> Option<JourneyVector>
where
    Segment: AsRef<[RawGPSPoint]> + ?Sized + 'a,
{
    let processed_data = raw_data.into_iter().flat_map(move |segment| {
        // we handle each segment separately
        let mut gps_preprocessor =
            segment_gap_rule_for_preprocessor.map(GpsPreprocessor::new_with_rule);

        let mut first = true;
        segment.as_ref().iter().map(move |raw_data| {
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

pub(crate) fn process_vector_data<'a, Segment>(
    raw_data: impl IntoIterator<Item = &'a Segment>,
    requested: ImportPreprocessor,
) -> JourneyData
where
    Segment: AsRef<[RawGPSPoint]> + ?Sized + 'a,
{
    let vector = match requested {
        ImportPreprocessor::None => {
            journey_vector_from_raw_data_with_gps_preprocessor(raw_data, None)
        }
        ImportPreprocessor::Generic => journey_vector_from_raw_data_with_gps_preprocessor(
            raw_data,
            Some(SegmentGapRule::Default),
        ),
        ImportPreprocessor::FlightTrack => flight_track_processor::process(raw_data),
        ImportPreprocessor::Spare => journey_vector_from_raw_data_with_gps_preprocessor(
            raw_data,
            Some(SegmentGapRule::Spare),
        ),
    }
    .unwrap_or_else(|| JourneyVector {
        track_segments: vec![],
    });
    JourneyData::Vector(vector)
}

pub fn journey_vector_from_raw_data_with_flight_track_processor(
    raw_data: &[Vec<RawGPSPoint>],
) -> Option<JourneyVector> {
    flight_track_processor::process(raw_data)
}

pub fn journey_info_from_raw_vector_data<'a, Segment>(
    raw_vector_data: impl IntoIterator<Item = &'a Segment>,
) -> JourneyInfo
where
    Segment: AsRef<[RawGPSPoint]> + ?Sized + 'a,
{
    let time_from_raw_data = |raw_data: &RawGPSPoint| {
        raw_data
            .timestamp_ms
            .and_then(|timestamp_ms| Utc.timestamp_millis_opt(timestamp_ms).single())
    };

    let mut journey_date_picker = JourneyDatePicker::new();
    for segment in raw_vector_data {
        for raw_data in segment.as_ref() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gps::Point;
    use crate::import_data::{ImportGrouping, ImportedJourney, ParsedVectorData};

    #[test]
    fn borrowed_groups_preserve_metadata_and_segment_boundaries() {
        let start = Local
            .with_ymd_and_hms(2026, 8, 11, 12, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let end = start + chrono::Duration::seconds(1);
        let point = |longitude, timestamp_ms| RawGPSPoint {
            point: Point {
                latitude: 0.0,
                longitude,
            },
            timestamp_ms,
            accuracy: None,
            altitude: None,
            speed: None,
        };
        let parsed = ParsedVectorData::new(
            ImportGrouping::ByJourney,
            vec![
                ImportedJourney::generic(vec![
                    Vec::new(),
                    vec![point(1.0, None), point(1.001, Some(end.timestamp_millis()))],
                ]),
                ImportedJourney::generic(vec![vec![
                    point(2.0, Some(start.timestamp_millis())),
                    point(2.001, Some(end.timestamp_millis())),
                ]]),
            ],
        );

        let info = journey_info_from_raw_vector_data(parsed.segments());
        assert_eq!(info.journey_date, start.with_timezone(&Local).date_naive());
        assert_eq!(info.start_time, Some(start));
        assert_eq!(info.end_time, Some(end));

        for processor in [
            ImportPreprocessor::None,
            ImportPreprocessor::Generic,
            ImportPreprocessor::Spare,
            ImportPreprocessor::FlightTrack,
        ] {
            let JourneyData::Vector(vector) = process_vector_data(parsed.segments(), processor)
            else {
                panic!("expected vector data");
            };
            let longitudes = vector
                .track_segments
                .iter()
                .map(|segment| {
                    segment
                        .track_points
                        .iter()
                        .map(|point| point.longitude)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                longitudes,
                [vec![1.0, 1.001], vec![2.0, 2.001]],
                "{processor:?}"
            );
        }
    }
}
