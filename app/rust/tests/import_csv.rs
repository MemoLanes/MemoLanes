use memolanes_core::api::import::{self as import_api, ImportPreprocessor};
use memolanes_core::import_data;
use std::fs;

#[test]
fn csv_sources_preserve_fields_and_use_the_expected_preprocessor() {
    use ImportPreprocessor::{Generic, Spare};
    for (file, count, timestamp, longitude, latitude, expected) in [
        (
            "DoL_laojunshan.csv",
            155,
            1_696_383_677_000,
            111.643623,
            33.718950,
            Spare,
        ),
        (
            "step.csv",
            2,
            1_767_196_805_000,
            117.118118,
            36.697596,
            Spare,
        ),
        (
            "raw_data.csv",
            929,
            1_754_726_365_283,
            -0.104277,
            51.520302,
            Generic,
        ),
    ] {
        eprintln!("CSV source: {file}");
        let path = format!("./tests/data/{file}");
        let (parsed, processor) = import_data::csv::load_csv(&path).unwrap();
        let segments = parsed;
        assert_eq!(
            std::mem::discriminant(&processor),
            std::mem::discriminant(&expected),
            "{file}"
        );
        assert_eq!(segments.len(), 1, "{file}");
        assert_eq!(segments[0].len(), count, "{file}");
        let first = &segments[0][0];
        assert_eq!(
            (
                first.timestamp_ms,
                first.point.longitude,
                first.point.latitude
            ),
            (Some(timestamp), longitude, latitude),
            "{file}"
        );
        let (info, raw, detected) = import_api::load_vector_data(path).unwrap();
        assert_eq!(
            std::mem::discriminant(&detected),
            std::mem::discriminant(&expected),
            "{file}"
        );
        let data = import_api::process_vector_data(&raw, detected).unwrap();
        assert!(!import_api::is_journey_data_empty(&data), "{file}");
        match file {
            "DoL_laojunshan.csv" => {
                assert_eq!(
                    segments[0].last().unwrap().timestamp_ms,
                    Some(1_696_386_813_000)
                );
                assert_eq!(info.journey_date.to_string(), "2023-10-04");
            }
            "step.csv" => {
                assert_eq!(
                    segments[0].last().unwrap().timestamp_ms,
                    Some(1_767_283_214_000)
                );
                assert_eq!(
                    (first.accuracy, first.altitude, first.speed),
                    (Some(5.35099), Some(49.254_64), None)
                );
            }
            "raw_data.csv" => assert_eq!(
                (first.accuracy, first.altitude, first.speed),
                (Some(5000.0), Some(0.0), Some(0.0))
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn invalid_csv_reports_the_cause() {
    let temp = tempdir::TempDir::new("invalid-csv").unwrap();
    let path = temp.path().join("track.csv");
    for (csv, expected_error) in [
        ("lat,lon,time\n36.697645,117.117744,1772523225\n", "unsupported CSV format"),
        ("timestamp,longitude,latitude,heading,accuracy,verticalAccuracy,speed,distance,altitude,course,param1,param2,param3\n1772523225,117.117744,91,326.305,14,3,0.049,0.000,49.491,326.305,0,0,\n", "invalid latitude 91 at CSV row 2"),
    ] {
        fs::write(&path, csv).unwrap();
        let error = import_data::csv::load_csv(path.to_str().unwrap()).unwrap_err();
        assert!(format!("{error:#}").contains(expected_error), "expected {expected_error}, got {error:#}");
    }
}
