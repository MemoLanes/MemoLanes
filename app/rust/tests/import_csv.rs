use memolanes_core::{
    api::import::{
        is_journey_data_empty, load_vector_data, process_vector_data, ImportPreprocessor,
    },
    import_data,
};

const LAOJUNSHAN_DOL_CSV: &str = "./tests/data/DoL_laojunshan.csv";

#[test]
fn loads_generated_laojunshan_dol_csv() {
    let (segments, preprocessor) = import_data::load_csv(LAOJUNSHAN_DOL_CSV).unwrap();

    assert!(matches!(preprocessor, ImportPreprocessor::Generic));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 155);

    let first = &segments[0][0];
    assert_eq!(first.timestamp_ms, Some(1_696_383_677_000));
    assert_eq!(first.point.longitude, 111.643623);
    assert_eq!(first.point.latitude, 33.718950);

    let last = segments[0].last().unwrap();
    assert_eq!(last.timestamp_ms, Some(1_696_386_813_000));
}

#[test]
fn vector_file_api_processes_generated_laojunshan_dol_csv() {
    let (journey_info, vector_data, preprocessor) =
        load_vector_data(LAOJUNSHAN_DOL_CSV.to_owned()).unwrap();

    assert_eq!(journey_info.journey_date.to_string(), "2023-10-04");
    assert!(matches!(preprocessor, ImportPreprocessor::Generic));

    let journey_data = process_vector_data(&vector_data, preprocessor).unwrap();
    assert!(!is_journey_data_empty(&journey_data));
}

#[test]
fn rejects_an_unregistered_csv_format() {
    let temp_dir = tempdir::TempDir::new("unsupported-csv").unwrap();
    let path = temp_dir.path().join("track.csv");
    std::fs::write(&path, "lat,lon,time\n36.697645,117.117744,1772523225\n").unwrap();

    let error = match import_data::load_csv(path.to_str().unwrap()) {
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

    let error = match import_data::load_csv(path.to_str().unwrap()) {
        Ok(_) => panic!("invalid coordinate should fail"),
        Err(error) => error,
    };

    assert!(format!("{error:#}").contains("invalid latitude 91 at CSV row 2"));
}
