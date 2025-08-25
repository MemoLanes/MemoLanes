pub mod test_utils;

use memolanes_core::gps_processor::{
    GpsPreprocessor, PathInterpolator, Point, ProcessResult, RawData,
};
use memolanes_core::{export_data, import_data};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::Path;

#[test]
fn first_data() {
    let mut gps_preprocessor = GpsPreprocessor::new();
    assert!(gps_preprocessor.last_kept_point().is_none());
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

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.163856
    );
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

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.1639266
    );
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

    assert_eq!(
        gps_preprocessor.last_kept_point().unwrap().latitude,
        120.163857
    );
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

fn run_though_test_data(name: &str) -> HashMap<ProcessResult, i32> {
    const GENERATE_RESULT_GPX_FOR_INSPECTION: bool = false;
    let mut gps_preprocessor = GpsPreprocessor::new();
    let mut counter = HashMap::new();
    let loaded_data = import_data::load_gpx(&format!("./tests/data/raw_gps_{}.gpx", name)).unwrap();
    for data in loaded_data.iter().flatten() {
        let result = gps_preprocessor.preprocess(data);
        counter.entry(result).and_modify(|c| *c += 1).or_insert(1);
    }

    if GENERATE_RESULT_GPX_FOR_INSPECTION {
        let journey_vector = import_data::journey_vector_from_raw_data(loaded_data, true).unwrap();

        let mut file = File::create(format!(
            "./tests/for_inspection/gps_preprocessor_run_though_test_data_{}.gpx",
            name
        ))
        .unwrap();
        export_data::journey_vector_to_gpx_file(&journey_vector, &mut file).unwrap();
    };

    counter
}

#[test]
fn run_though_test_data_shanghai() {
    let counter = run_though_test_data("shanghai");
    assert_eq!(counter[&ProcessResult::NewSegment], 11);
    assert_eq!(counter[&ProcessResult::Append], 2959);
    assert_eq!(counter[&ProcessResult::Ignore], 648);
}

#[test]
fn run_though_test_data_shenzhen_stationary() {
    let counter = run_though_test_data("shenzhen_stationary");
    assert_eq!(counter[&ProcessResult::NewSegment], 8);
    assert_eq!(counter[&ProcessResult::Append], 443);
    assert_eq!(counter[&ProcessResult::Ignore], 6279);
}

#[test]
fn run_though_test_data_laojunshan() {
    let counter = run_though_test_data("laojunshan");
    assert_eq!(counter[&ProcessResult::NewSegment], 2);
    assert_eq!(counter[&ProcessResult::Append], 2595);
    assert_eq!(counter[&ProcessResult::Ignore], 348);
}

#[test]
fn run_interpolate_test() {
    let interpolator = PathInterpolator::new();

    let names = get_filenames_without_extension("./tests/data/interpolate/");

    for name in &names.unwrap() {
        let loaded_data =
            import_data::load_kml(&format!("./tests/data/interpolate/{}.kml", name)).unwrap();

        let result = interpolator.interpolate(&loaded_data[0]);
        let mut file =
            File::create(format!("./tests/for_inspection/interpolate/{}.gpx", name)).unwrap();
        export_data::journey_vector_to_gpx_file(&result, &mut file).unwrap();
    }
}

fn get_filenames_without_extension(folder_path: &str) -> Result<Vec<String>, std::io::Error> {
    let path = Path::new(folder_path);
    let mut filenames = Vec::new();

    // 读取目录中的所有条目
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        // 只处理文件，跳过目录
        if path.is_file() {
            // 获取文件名
            if let Some(file_name) = path.file_name() {
                if let Some(name_str) = file_name.to_str() {
                    // 去除文件后缀
                    let name_without_ext = if let Some(dot_pos) = name_str.rfind('.') {
                        &name_str[..dot_pos]
                    } else {
                        name_str
                    };
                    filenames.push(name_without_ext.to_string());
                }
            }
        }
    }
    Ok(filenames)
}
