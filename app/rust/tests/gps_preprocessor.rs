pub mod test_utils;

use memolanes_core::gps_processor::{GpsPreprocessor, Point, ProcessResult, RawData};
use std::collections::HashMap;

#[test]
fn first_data() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    assert!(gps_preprocessor.last_point().is_none());
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );
}

#[test]
fn ignore() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(300.0),
        altitude: Some(10.),
        speed: Some(0.6028665),
    };
    assert_eq!(gps_preprocessor.preprocess(&data), ProcessResult::Ignore);
}

#[test]
fn time_difference() {
    let mut gps_preprocessor = GpsPreprocessor::new();

    gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    });

    assert_eq!(gps_preprocessor.last_point().unwrap().latitude, 120.163856);
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.1639266,
            longitude: 30.271981,
        },
        timestamp_ms: Some(1697349117449),
        accuracy: Some(3.5),
        altitude: Some(20.),
        speed: Some(0.18825254),
    });
    assert_eq!(ProcessResult::Append, result);

    assert_eq!(gps_preprocessor.last_point().unwrap().latitude, 120.1639266);
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163857,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1698349116449),
        accuracy: Some(3.9),
        altitude: Some(30.),
        speed: Some(0.6028665),
    });
    assert_eq!(ProcessResult::NewSegment, result);

    assert_eq!(gps_preprocessor.last_point().unwrap().latitude, 120.163857);
    let result = gps_preprocessor.preprocess(&RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116449),
        accuracy: Some(3.9),
        altitude: Some(10.),
        speed: Some(0.6028665),
    });
    assert_eq!(ProcessResult::Ignore, result);
}

#[test]
fn speed() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    let data = RawData {
        point: Point {
            latitude: 120.163856,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349116000),
        accuracy: None,
        altitude: None,
        speed: None,
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );

    let data = RawData {
        point: Point {
            latitude: 125.0,
            longitude: 30.2719716,
        },
        timestamp_ms: Some(1697349117000),
        accuracy: None,
        altitude: None,
        speed: None,
    };
    assert_eq!(
        gps_preprocessor.preprocess(&data),
        ProcessResult::NewSegment
    );
}

#[test]
fn run_though_test_data() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    let mut counter = HashMap::new();
    for data in test_utils::load_raw_gpx_data_for_test() {
        let result = gps_preprocessor.preprocess(&data);
        counter.entry(result).and_modify(|c| *c += 1).or_insert(1);
    }
    assert_eq!(counter[&ProcessResult::NewSegment], 8);
    assert_eq!(counter[&ProcessResult::Append], 2670);
    assert_eq!(counter[&ProcessResult::Ignore], 940);
}
