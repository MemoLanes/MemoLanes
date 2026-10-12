use chrono::{Local, NaiveDate, TimeZone};
use memolanes_core::{
    api::import::{
        analyze_vector_data_parts, import_vector_data_by_parts, is_journey_data_empty,
        load_vector_data, process_vector_data, process_vector_data_for_part, ImportPreprocessor,
        RawVectorData,
    },
    gps::{Point, RawGPSPoint},
    import_data::{
        csv,
        journey_partition::{group_by_date, summarize},
    },
    journey_header::JourneyKind,
};

fn point(timestamp_ms: Option<i64>, longitude: f64) -> RawGPSPoint {
    RawGPSPoint {
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

fn load_csv_vector_data(
    name: &str,
    rows: &str,
    journey_ids: bool,
) -> (tempdir::TempDir, RawVectorData, ImportPreprocessor) {
    let temp_dir = tempdir::TempDir::new(name).unwrap();
    let csv_path = temp_dir.path().join("data.csv");
    let id_column = if journey_ids { ",journey_id" } else { "" };
    std::fs::write(
        &csv_path,
        format!(
            "timestamp_ms,received_timestamp_ms,latitude,longitude,accuracy,altitude,speed{id_column}\n{rows}"
        ),
    )
    .unwrap();
    let (_, vector_data, preprocessor) =
        load_vector_data(csv_path.to_string_lossy().into_owned()).unwrap();
    (temp_dir, vector_data, preprocessor)
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
    let (_temp, data, _) = load_csv_vector_data("untimed-preview", ",0,0,1,,,\n", false);
    assert!(analyze_vector_data_parts(&data).is_empty());
    let preview = process_vector_data(&data, ImportPreprocessor::None).unwrap();
    assert!(!is_journey_data_empty(&preview));
}

#[test]
fn api_uses_partitioned_data() {
    let day_one = NaiveDate::from_ymd_opt(2026, 8, 11).unwrap();
    let day_two = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let timestamp_one = local_timestamp(2026, 8, 11, 23, 59, 59);
    let timestamp_two = local_timestamp(2026, 8, 12, 0, 30, 0);
    let (_temp, vector_data, preprocessor) = load_csv_vector_data(
        "vector-import-by-date",
        &format!(
            "{timestamp_one},{timestamp_one},0,1,,,\n\
             ,{timestamp_one},0,2,,,\n\
             {timestamp_two},{timestamp_two},0,3,,,\n"
        ),
        false,
    );
    let parts = analyze_vector_data_parts(&vector_data);

    assert_eq!(parts.len(), 2);
    assert!(parts.iter().all(|part| !part.is_explicit_journey));
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
            process_vector_data_for_part(&vector_data, date.to_string(), preprocessor).unwrap();
        assert!(!is_journey_data_empty(&journey_data));
    }

    // Explicit IDs survive same-date grouping, interleaving and missing GPS times.
    let timestamp = local_timestamp(2026, 8, 11, 12, 0, 0);
    let (temp, vector_data, preprocessor) = load_csv_vector_data(
        "vector-import-by-id",
        &format!(
            "{timestamp},{timestamp},0,1,,,,A\n\
             {timestamp},{timestamp},0,10,,,,B\n\
             {next},{next},0,11,,,,B\n\
             {last},{last},0,2,,,,A\n\
             ,{timestamp},0,20,100,,,C\n\
             ,{timestamp},0,21,100,,,C\n",
            next = timestamp + 60_000,
            last = timestamp + 120_000,
        ),
        true,
    );
    let (parsed, _) = csv::load_csv(temp.path().join("data.csv").to_str().unwrap()).unwrap();
    assert_eq!(parsed.groups()[0].segments.len(), 2);
    assert_eq!(parsed.groups()[1].segments.len(), 1);
    let parts = analyze_vector_data_parts(&vector_data);
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|part| part.is_explicit_journey
        && !part.is_memolanes_journey
        && part.point_count == 2));
    assert_eq!(parts[0].journey_date, parts[1].journey_date);
    assert_ne!(parts[0].part_key, parts[1].part_key);
    assert_eq!(parts[2].missing_timestamp_count, 2);
    assert_eq!(parts[2].start_time, None);
    assert_eq!(parts[2].end_time, None);
    for part in &parts {
        let data = process_vector_data_for_part(
            &vector_data,
            part.part_key.clone(),
            ImportPreprocessor::None,
        )
        .unwrap();
        assert!(!is_journey_data_empty(&data));
    }
    let filtered =
        process_vector_data_for_part(&vector_data, parts[2].part_key.clone(), preprocessor)
            .unwrap();
    assert!(is_journey_data_empty(&filtered));
}

#[test]
fn api_rejects_unknown_dates() {
    let timestamp = local_timestamp(2026, 8, 11, 12, 0, 0);
    let (_temp, vector_data, preprocessor) = load_csv_vector_data(
        "vector-import-invalid-date",
        &format!("{timestamp},{timestamp},0,1,,,\n"),
        false,
    );
    let error = import_vector_data_by_parts(
        &vector_data,
        vec!["2026-08-12".to_owned()],
        preprocessor,
        JourneyKind::DefaultKind,
        None,
    )
    .unwrap_err();

    assert!(format!("{error:#}").contains("No vector data for parts: [\"2026-08-12\"]"));
}
