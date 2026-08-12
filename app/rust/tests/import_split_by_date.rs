use chrono::{Local, NaiveDate, TimeZone};
use memolanes_core::{
    api::import::{
        analyze_vector_data_by_date, import_vector_data_by_date, is_journey_data_empty,
        load_vector_data, process_vector_data_for_date,
    },
    gps_processor::{Point, RawData},
    import_data::split_by_date::split_raw_vector_data_by_local_date,
    journey_header::JourneyKind,
};

fn point(timestamp_ms: Option<i64>, longitude: f64) -> RawData {
    RawData {
        point: Point {
            latitude: 0.0,
            longitude,
        },
        timestamp_ms,
        accuracy: None,
        altitude: None,
        speed: None,
    }
}

fn local_timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    Local
        .with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .unwrap()
        .timestamp_millis()
}

#[test]
fn splits_at_local_midnight_and_preserves_source_segments() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let raw_data = vec![
        vec![
            point(Some(local_timestamp(2026, 8, 11, 23, 59, 59)), 1.0),
            point(Some(local_timestamp(2026, 8, 12, 0, 0, 0)), 2.0),
        ],
        vec![point(Some(local_timestamp(2026, 8, 12, 12, 0, 0)), 3.0)],
    ];

    let split = split_raw_vector_data_by_local_date(&raw_data);
    assert_eq!(split.len(), 2);
    assert_eq!(split[&day_one].len(), 1);
    assert_eq!(split[&day_two].len(), 2);
    assert_eq!(split[&day_two].iter().map(Vec::len).sum::<usize>(), 2);
}

#[test]
fn assigns_missing_timestamps_without_losing_points() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let raw_data = vec![
        vec![point(None, 1.0)],
        vec![
            point(None, 2.0),
            point(Some(local_timestamp(2026, 8, 11, 23, 59, 59)), 3.0),
            point(None, 4.0),
            point(Some(local_timestamp(2026, 8, 12, 0, 0, 0)), 5.0),
            point(None, 6.0),
        ],
        vec![point(None, 7.0)],
    ];

    let split = split_raw_vector_data_by_local_date(&raw_data);
    assert_eq!(split.len(), 2);
    assert_eq!(
        split.values().flatten().map(Vec::len).sum::<usize>(),
        raw_data.iter().map(Vec::len).sum::<usize>()
    );
    assert_eq!(split[&day_one].len(), 2);
    assert_eq!(split[&day_two].len(), 2);
}

#[test]
fn cannot_split_fully_untimed_data() {
    assert!(split_raw_vector_data_by_local_date(&[vec![point(None, 1.0)]]).is_empty());
}

#[test]
fn vector_import_api_reuses_split_data_and_returns_string_dates() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let temp_dir = tempdir::TempDir::new("vector-import-by-date").unwrap();
    let csv_path = temp_dir.path().join("multi-day.csv");
    let timestamp_one = local_timestamp(2026, 8, 11, 23, 59, 59);
    let timestamp_two = local_timestamp(2026, 8, 12, 0, 0, 0);
    std::fs::write(
        &csv_path,
        format!(
            "timestamp_ms,received_timestamp_ms,latitude,longitude,accuracy,altitude,speed\n\
             {timestamp_one},{timestamp_one},0,1,,,\n\
             ,{timestamp_one},0,2,,,\n\
             {timestamp_two},{timestamp_two},0,3,,,\n"
        ),
    )
    .unwrap();

    let (_, vector_data, preprocessor) =
        load_vector_data(csv_path.to_string_lossy().into_owned()).unwrap();
    let parts = analyze_vector_data_by_date(&vector_data);

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].journey_date, day_one.to_string());
    assert_eq!(parts[0].point_count, 2);
    assert_eq!(parts[0].missing_timestamp_count, 1);
    assert_eq!(parts[1].journey_date, day_two.to_string());
    assert_eq!(parts[1].point_count, 1);
    assert_eq!(parts[1].missing_timestamp_count, 0);

    for date in [day_one, day_two] {
        let journey_data =
            process_vector_data_for_date(&vector_data, date.to_string(), preprocessor).unwrap();
        assert!(!is_journey_data_empty(&journey_data));
    }
}

#[test]
fn vector_import_rejects_dates_that_are_not_in_the_loaded_data() {
    let temp_dir = tempdir::TempDir::new("vector-import-invalid-date").unwrap();
    let csv_path = temp_dir.path().join("single-day.csv");
    let timestamp = local_timestamp(2026, 8, 11, 12, 0, 0);
    std::fs::write(
        &csv_path,
        format!(
            "timestamp_ms,received_timestamp_ms,latitude,longitude,accuracy,altitude,speed\n\
             {timestamp},{timestamp},0,1,,,\n"
        ),
    )
    .unwrap();

    let (_, vector_data, preprocessor) =
        load_vector_data(csv_path.to_string_lossy().into_owned()).unwrap();
    let error = import_vector_data_by_date(
        &vector_data,
        vec!["2026-08-12".to_owned()],
        preprocessor,
        JourneyKind::DefaultKind,
        None,
    )
    .unwrap_err();

    assert!(format!("{error:#}").contains("No vector data for dates: [2026-08-12]"));
}
