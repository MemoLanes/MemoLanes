use native::gps_processor::{GpsProcessor, ProcessResult, RawData};
mod load_test_data;

#[test]
fn basic() {
    assert_eq!(ProcessResult::NewSegment.to_int(), 1);
    assert_eq!(ProcessResult::Ignore.to_int(), -1);
    assert_eq!(ProcessResult::Append.to_int(), 0);
}
#[test]
fn test_first_data() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    };
    assert_eq!(
        ProcessResult::NewSegment.to_int(),
        gps_processor.process(&data).to_int()
    );
}
#[test]
fn test_accuracy() {
    let mut gps_processor = GpsProcessor::new();
    let data = RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 300.0,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    };
    assert_eq!(
        ProcessResult::Ignore.to_int(),
        gps_processor.process(&data).to_int()
    );
}
#[test]
fn test_time_diff() {
    let mut vec: Vec<RawData> = Vec::new();
    let mut gps_processor = GpsProcessor::new();
    vec.push(RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    });
    vec.push(RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1698349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    });
    let mut last_process_result: Option<ProcessResult> = None;
    for data in vec.iter() {
        last_process_result = Some(gps_processor.process(&data))
    }

    assert_eq!(
        ProcessResult::NewSegment.to_int(),
        last_process_result.unwrap().to_int()
    );
}

#[test]
fn test_append() {
    let mut vec: Vec<RawData> = Vec::new();
    let mut gps_processor = GpsProcessor::new();
    vec.push(RawData {
        latitude: 120.163856,
        longitude: 30.2719716,
        timestamp_ms: 1697349116449,
        accuracy: 3.9,
        altitude: Some(72.9),
        speed: Some(0.6028665),
    });
    vec.push(RawData {
        latitude: 120.1639266,
        longitude: 30.271981,
        timestamp_ms: 1697349122420,
        accuracy: 3.9,
        altitude: Some(75.6),
        speed: Some(0.18825254),
    });
    let mut last_process_result: Option<ProcessResult> = None;
    for data in vec.iter() {
        last_process_result = Some(gps_processor.process(&data))
    }

    assert_eq!(
        ProcessResult::Append.to_int(),
        last_process_result.unwrap().to_int()
    );
}
