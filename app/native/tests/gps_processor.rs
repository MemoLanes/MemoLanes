pub mod test_utils;

use native::gps_processor::{GpsProcessor, ProcessResult, RawData};
use std::collections::HashMap;

#[test]
fn first_data() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    gps_processor.process(data, |last_data, curr_data, result| {
        assert!(last_data.is_none());
        assert_eq!(curr_data.altitude.unwrap(), 10.);
        assert_eq!(ProcessResult::NewSegment, result);
    });
}

#[test]
fn ignore() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 300.0,
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    gps_processor.process(data, |_, _, result| {
        assert_eq!(ProcessResult::Ignore, result);
    });
}

#[test]
fn time_difference() {
    let mut gps_processor = GpsProcessor::new();

    gps_processor.process(
        RawData {
            latitude: 120.163856,
            longitude: 30.2719716,
            timestamp_ms: 1697349116449,
            accuracy: 3.9,
            altitude: Some(10.),
            speed: Some(0.6028665),
        },
        |_, _, _| {},
    );

    gps_processor.process(
        RawData {
            latitude: 120.1639266,
            longitude: 30.271981,
            timestamp_ms: 1697349117449,
            accuracy: 3.5,
            altitude: Some(20.),
            speed: Some(0.18825254),
        },
        |last_data, curr_data, result| {
            assert_eq!(last_data.as_ref().unwrap().altitude.unwrap(), 10.);
            assert_eq!(curr_data.altitude.unwrap(), 20.);
            assert_eq!(ProcessResult::Append, result);
        },
    );

    gps_processor.process(
        RawData {
            latitude: 120.163856,
            longitude: 30.2719716,
            timestamp_ms: 1698349116449,
            accuracy: 3.9,
            altitude: Some(30.),
            speed: Some(0.6028665),
        },
        |last_data, curr_data, result| {
            assert_eq!(last_data.as_ref().unwrap().altitude.unwrap(), 20.);
            assert_eq!(curr_data.altitude.unwrap(), 30.);
            assert_eq!(ProcessResult::NewSegment, result);
        },
    );
}

#[test]
fn run_though_test_data() {
    let mut gps_processor = GpsProcessor::new();
    let mut counter = HashMap::new();
    for data in test_utils::load_raw_gpx_data_for_test() {
        gps_processor.process(data, |_, _, result| {
            counter.entry(result).and_modify(|c| *c += 1).or_insert(1);
        });
    }
    assert_eq!(counter[&ProcessResult::NewSegment], 8);
    assert_eq!(counter[&ProcessResult::Append], 3574);
    assert_eq!(counter[&ProcessResult::Ignore], 36);
}
