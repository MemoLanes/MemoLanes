use native::gps_processor::{GpsProcessor, ProcessResult};
mod load_test_data;

#[test]
fn basic() {
    let test_data = load_test_data::load_raw_gpx_data_for_test();
    let num_of_gpx_data_in_input = test_data.len();
    print!("total test data: {}\n", num_of_gpx_data_in_input);
    let mut gps_processor = GpsProcessor::new();
    for (i, raw_data) in test_data.iter().enumerate() {
        let process_result = gps_processor.process(raw_data);
        print!("{},", process_result.to_int())
    }
    assert_eq!(ProcessResult::NewSegment.to_int(), 1);
    assert_eq!(ProcessResult::Ignore.to_int(), -1);
}
