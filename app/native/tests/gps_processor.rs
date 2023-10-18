use native::gps_processor::{GpsProcessor, ProcessResult, RawData};
use std::collections::HashMap;
mod load_test_data;

#[test]
fn first_data() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    };
    assert_eq!(ProcessResult::NewSegment, gps_processor.process(&data));
}

#[test]
fn ignore() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 300.0,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    };
    assert_eq!(ProcessResult::Ignore, gps_processor.process(&data));
}

#[test]
fn time_difference() {
    let mut gps_processor = GpsProcessor::new();
    gps_processor.process(&RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    });

    let result = gps_processor.process(&RawData {
        latitude: 120.1639266,
        longitude: 30.271981,
        timestamp_ms: 1697349117449,
        accuracy: 3.5,
        altitude: Some(75.6),
        speed: Some(0.18825254),
    });
    assert_eq!(ProcessResult::Append, result);

    let result = gps_processor.process(&RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1698349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    });
    assert_eq!(ProcessResult::NewSegment, result);
}

#[test]
fn run_though_test_data() {
    let mut gps_processor = GpsProcessor::new();
    let mut counter = HashMap::new();
    for data in load_test_data::load_raw_gpx_data_for_test() {
        let result = gps_processor.process(&data);
        counter.entry(result).and_modify(|c| *c += 1).or_insert(1);
    }
    assert_eq!(counter[&ProcessResult::NewSegment], 8);
    assert_eq!(counter[&ProcessResult::Append], 3574);
    assert_eq!(counter[&ProcessResult::Ignore], 36);
}
