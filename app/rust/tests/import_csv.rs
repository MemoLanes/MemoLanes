use memolanes_core::{
    api::import::{is_journey_data_empty, load_vector_data, process_vector_data},
    import_data::{self, ImportPreprocessor},
    journey_header::JourneyInfo,
};

const LAOJUNSHAN_DOL_CSV: &str = "./tests/data/DoL_laojunshan.csv";
const STEP_CSV: &str = "./tests/data/step.csv";
const MEMOLANES_CSV: &str = "./tests/data/raw_data.csv";

fn assert_csv_api_processes(path: &str, expected: ImportPreprocessor) -> JourneyInfo {
    let (info, vector_data, preprocessor) = load_vector_data(path.to_owned()).unwrap();
    assert_eq!(
        std::mem::discriminant(&preprocessor),
        std::mem::discriminant(&expected),
        "{path}: unexpected preprocessor {preprocessor:?}",
    );
    let journey_data = process_vector_data(&vector_data, preprocessor).unwrap();
    assert!(!is_journey_data_empty(&journey_data), "{path}");
    info
}

#[test]
fn imports_generated_laojunshan_dol_csv() {
    let (parsed, preprocessor) = import_data::csv::load_csv(LAOJUNSHAN_DOL_CSV).unwrap();
    let segments = parsed.flatten();

    assert!(matches!(preprocessor, ImportPreprocessor::Spare));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 155);

    let first = &segments[0][0];
    assert_eq!(first.timestamp_ms, Some(1_696_383_677_000));
    assert_eq!(first.point.longitude, 111.643623);
    assert_eq!(first.point.latitude, 33.718950);

    let last = segments[0].last().unwrap();
    assert_eq!(last.timestamp_ms, Some(1_696_386_813_000));
    let journey_info = assert_csv_api_processes(LAOJUNSHAN_DOL_CSV, preprocessor);
    assert_eq!(journey_info.journey_date.to_string(), "2023-10-04");
}

#[test]
fn imports_step_csv() {
    let (parsed, preprocessor) = import_data::csv::load_csv(STEP_CSV).unwrap();
    let segments = parsed.flatten();

    assert!(matches!(preprocessor, ImportPreprocessor::Spare));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 2);

    let first = &segments[0][0];
    assert_eq!(first.timestamp_ms, Some(1_767_196_805_000));
    assert_eq!(first.point.longitude, 117.118118);
    assert_eq!(first.point.latitude, 36.697596);
    assert_eq!(first.accuracy, Some(5.35099));
    assert_eq!(first.altitude, Some(49.254637));
    assert_eq!(first.speed, None);

    let last = segments[0].last().unwrap();
    assert_eq!(last.timestamp_ms, Some(1_767_283_214_000));
    assert_csv_api_processes(STEP_CSV, preprocessor);
}

#[test]
fn imports_memolanes_csv() {
    let (parsed, preprocessor) = import_data::csv::load_csv(MEMOLANES_CSV).unwrap();
    let segments = parsed.flatten();

    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 929);

    let first = &segments[0][0];
    assert_eq!(first.timestamp_ms, Some(1_754_726_365_283));
    assert_eq!(first.point.longitude, -0.104277);
    assert_eq!(first.point.latitude, 51.520302);
    assert_eq!(first.accuracy, Some(5000.0));
    assert_eq!(first.altitude, Some(0.0));
    assert_eq!(first.speed, Some(0.0));
    assert_csv_api_processes(MEMOLANES_CSV, preprocessor);
}

#[test]
fn rejects_an_unregistered_csv_format() {
    let temp_dir = tempdir::TempDir::new("unsupported-csv").unwrap();
    let path = temp_dir.path().join("track.csv");
    std::fs::write(&path, "lat,lon,time\n36.697645,117.117744,1772523225\n").unwrap();

    let error = match import_data::csv::load_csv(path.to_str().unwrap()) {
        Ok(_) => panic!("unsupported CSV should fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("unsupported CSV format"));
}

#[test]
fn reports_invalid_coordinates_with_the_row_number() {
    let temp_dir = tempdir::TempDir::new("invalid-csv-coordinate").unwrap();
    let path = temp_dir.path().join("track.csv");
    std::fs::write(
        &path,
        "timestamp,longitude,latitude,heading,accuracy,verticalAccuracy,speed,distance,altitude,course,param1,param2,param3\n\
         1772523225,117.117744,91,326.305,14,3,0.049,0.000,49.491,326.305,0,0,\n",
    )
    .unwrap();

    let error = match import_data::csv::load_csv(path.to_str().unwrap()) {
        Ok(_) => panic!("invalid coordinate should fail"),
        Err(error) => error,
    };

    assert!(format!("{error:#}").contains("invalid latitude 91 at CSV row 2"));
}
