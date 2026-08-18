use chrono::{Local, NaiveDate, TimeZone};
use memolanes_core::{
    api::import::{
        analyze_vector_data_by_date, import_vector_data_by_date, is_journey_data_empty,
        load_vector_data, process_vector_data_for_date, ImportPreprocessor, RawVectorData,
    },
    gps_processor::{Point, RawData},
    import_data::journey_partition::{group_by_date, summarize},
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

fn load_csv_vector_data(name: &str, rows: &str) -> (RawVectorData, ImportPreprocessor) {
    let temp_dir = tempdir::TempDir::new(name).unwrap();
    let csv_path = temp_dir.path().join("data.csv");
    std::fs::write(
        &csv_path,
        format!(
            "timestamp_ms,received_timestamp_ms,latitude,longitude,accuracy,altitude,speed\n{rows}"
        ),
    )
    .unwrap();
    let (_, vector_data, preprocessor) =
        load_vector_data(csv_path.to_string_lossy().into_owned()).unwrap();
    (vector_data, preprocessor)
}

#[test]
fn continuous_midnight_track_stays_on_previous_day() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let raw_data = vec![
        vec![
            point(Some(local_timestamp(2026, 8, 11, 23, 50, 0)), 0.0),
            point(Some(local_timestamp(2026, 8, 11, 23, 59, 0)), 2.0),
            point(Some(local_timestamp(2026, 8, 12, 0, 0, 0)), 2.01),
        ],
        vec![point(Some(local_timestamp(2026, 8, 12, 0, 10, 0)), 2.02)],
    ];

    let parts = group_by_date(&raw_data);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[&day_one], raw_data);
}

#[test]
fn cross_midnight_idle_gap_splits() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let raw_data = vec![vec![
        point(Some(local_timestamp(2026, 8, 11, 23, 50, 0)), 0.0),
        point(Some(local_timestamp(2026, 8, 11, 23, 59, 0)), 1.0),
        point(Some(local_timestamp(2026, 8, 12, 0, 30, 0)), 2.0),
    ]];

    let parts = group_by_date(&raw_data);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[&day_one], vec![raw_data[0][..2].to_vec()]);
    assert_eq!(parts[&day_two], vec![raw_data[0][2..].to_vec()]);
}

#[test]
fn missing_timestamps_are_preserved() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let raw_data = vec![
        vec![point(None, 1.0)],
        vec![
            point(None, 2.0),
            point(Some(local_timestamp(2026, 8, 11, 23, 59, 59)), 3.0),
            point(None, 4.0),
            point(Some(local_timestamp(2026, 8, 12, 0, 30, 0)), 5.0),
            point(None, 6.0),
        ],
        vec![point(None, 7.0)],
    ];

    let parts = group_by_date(&raw_data);
    assert_eq!(parts.len(), 2);
    assert_eq!(
        parts.values().flatten().map(Vec::len).sum::<usize>(),
        raw_data.iter().map(Vec::len).sum::<usize>()
    );
    assert_eq!(parts[&day_one].len(), 2);
    assert_eq!(parts[&day_two].len(), 2);

    let summaries = summarize(&raw_data);
    assert_eq!(summaries[&day_one].point_count, 4);
    assert_eq!(summaries[&day_one].missing_timestamp_count, 3);
    assert_eq!(summaries[&day_two].point_count, 3);
    assert_eq!(summaries[&day_two].missing_timestamp_count, 2);
}

#[test]
fn fully_untimed_data_cannot_be_grouped() {
    assert!(group_by_date(&[vec![point(None, 1.0)]]).is_empty());
}

#[test]
fn api_uses_partitioned_data() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let timestamp_one = local_timestamp(2026, 8, 11, 23, 59, 59);
    let timestamp_two = local_timestamp(2026, 8, 12, 0, 30, 0);
    let (vector_data, preprocessor) = load_csv_vector_data(
        "vector-import-by-date",
        &format!(
            "{timestamp_one},{timestamp_one},0,1,,,\n\
             ,{timestamp_one},0,2,,,\n\
             {timestamp_two},{timestamp_two},0,3,,,\n"
        ),
    );
    let parts = analyze_vector_data_by_date(&vector_data);

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].journey_date, day_one.to_string());
    assert_eq!(
        parts[0].start_time.unwrap().timestamp_millis(),
        timestamp_one
    );
    assert_eq!(parts[0].end_time.unwrap().timestamp_millis(), timestamp_one);
    assert_eq!(parts[0].point_count, 2);
    assert_eq!(parts[0].missing_timestamp_count, 1);
    assert_eq!(parts[1].journey_date, day_two.to_string());
    assert_eq!(
        parts[1].start_time.unwrap().timestamp_millis(),
        timestamp_two
    );
    assert_eq!(parts[1].end_time.unwrap().timestamp_millis(), timestamp_two);
    assert_eq!(parts[1].point_count, 1);
    assert_eq!(parts[1].missing_timestamp_count, 0);

    for date in [day_one, day_two] {
        let journey_data =
            process_vector_data_for_date(&vector_data, date.to_string(), preprocessor).unwrap();
        assert!(!is_journey_data_empty(&journey_data));
    }
}

#[test]
fn api_rejects_unknown_dates() {
    let timestamp = local_timestamp(2026, 8, 11, 12, 0, 0);
    let (vector_data, preprocessor) = load_csv_vector_data(
        "vector-import-invalid-date",
        &format!("{timestamp},{timestamp},0,1,,,\n"),
    );
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
