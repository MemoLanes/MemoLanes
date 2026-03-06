use chrono::{Datelike, NaiveDate};
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use memolanes_core::{
    cache_db::LayerKind, import_data, journey_bitmap::JourneyBitmap, journey_data::JourneyData,
    journey_header::JourneyKind, storage::Storage,
};

fn last_day_of_month(date: NaiveDate) -> NaiveDate {
    let (y, m) = (date.year(), date.month());
    if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    }
    .unwrap()
    .pred_opt()
    .unwrap()
}
use std::time::Duration;
use tempdir::TempDir;

// ---------------------------------------------------------------------------
// Data sources
// ---------------------------------------------------------------------------

struct DataSource {
    label: &'static str,
    years: i32,
    journeys_per_month: u32,
}

fn hash32(x: u32) -> u32 {
    let x = x ^ (x >> 16);
    let x = x.wrapping_mul(0x45d9f3b);
    x ^ (x >> 16)
}

fn make_synthetic_bitmap(seed: u32) -> JourneyBitmap {
    let h = hash32(seed);
    let start_lng = 151.0 + (h & 0xFF) as f64 / 255.0 * 0.5;
    let start_lat = -34.0 + ((h >> 8) & 0xFF) as f64 / 255.0 * 0.5;
    let end_lng = 151.0 + ((h >> 16) & 0xFF) as f64 / 255.0 * 0.5;
    let end_lat = -34.0 + ((h >> 24) & 0xFF) as f64 / 255.0 * 0.5;
    let mut bm = JourneyBitmap::new();
    bm.add_line(start_lng, start_lat, end_lng, end_lat);
    bm
}

fn data_sources() -> Vec<DataSource> {
    vec![
        DataSource {
            label: "small_12mo",
            years: 1,
            journeys_per_month: 30,
        },
        DataSource {
            label: "large_120mo",
            years: 10,
            journeys_per_month: 30,
        },
    ]
}

// ---------------------------------------------------------------------------
// Helper: populate storage with data from a source
// ---------------------------------------------------------------------------

fn populate_storage(storage: &Storage, src: &DataSource) {
    let base_year = 2019i32;
    storage
        .with_db_txn(|txn| {
            for yr in 0..src.years {
                for month in 1u32..=12 {
                    for j in 0..src.journeys_per_month {
                        let day = (j % 28) + 1;
                        let date = NaiveDate::from_ymd_opt(base_year + yr, month, day).unwrap();
                        txn.create_and_insert_journey(
                            date,
                            None,
                            None,
                            None,
                            JourneyKind::DefaultKind,
                            None,
                            JourneyData::Bitmap(make_synthetic_bitmap(
                                month * src.journeys_per_month + j,
                            )),
                        )?;
                    }
                }
            }
            Ok(())
        })
        .unwrap();
}

// ---------------------------------------------------------------------------
// Helper: create a fresh storage with a given cache config and data source
// ---------------------------------------------------------------------------

fn setup_storage_with(src: &DataSource) -> (Storage, TempDir, TempDir, TempDir, TempDir) {
    let temp_dir = TempDir::new("bench-temp").unwrap();
    let doc_dir = TempDir::new("bench-doc").unwrap();
    let support_dir = TempDir::new("bench-support").unwrap();
    let cache_dir = TempDir::new("bench-cache").unwrap();

    let storage = Storage::init(
        temp_dir.path().to_str().unwrap().to_string(),
        doc_dir.path().to_str().unwrap().to_string(),
        support_dir.path().to_str().unwrap().to_string(),
        cache_dir.path().to_str().unwrap().to_string(),
    );

    populate_storage(&storage, src);

    (storage, temp_dir, doc_dir, support_dir, cache_dir)
}

// ---------------------------------------------------------------------------
// Helper: load a small sample bitmap from the test GPX fixture
// ---------------------------------------------------------------------------

fn load_sample_bitmap() -> JourneyBitmap {
    let (raw_data, _) = import_data::load_gpx("./tests/data/raw_gps_shanghai.gpx").unwrap();
    let vector =
        import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data, None).unwrap();
    let mut bitmap = JourneyBitmap::new();
    bitmap.merge_vector(&vector);
    bitmap
}

// ---------------------------------------------------------------------------
// Helper: derive representative date ranges from actual DB contents
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
struct DateRanges {
    sample_date: NaiveDate,
    week_start: NaiveDate,
    week_end: NaiveDate,
    month_start: NaiveDate,
    month_end: NaiveDate,
    three_months_start: NaiveDate,
    three_months_end: NaiveDate,
    cross_month_start: NaiveDate,
    cross_month_end: NaiveDate,
    year_start: NaiveDate,
    year_end: NaiveDate,
}

fn determine_date_ranges(storage: &Storage) -> DateRanges {
    storage
        .with_db_txn(|txn| {
            let years = txn.years_with_journey()?;
            assert!(!years.is_empty(), "no journeys found in storage");

            // Year with the most months of data
            let target_year = years
                .iter()
                .copied()
                .max_by_key(|&y| txn.months_with_journey(y).map(|m| m.len()).unwrap_or(0))
                .unwrap();

            let months_i32 = txn.months_with_journey(target_year)?;
            assert!(!months_i32.is_empty());
            let months: Vec<u32> = months_i32.iter().map(|&m| m as u32).collect();

            let days_i32 = txn.days_with_journey(target_year, months[0] as i32)?;
            assert!(!days_i32.is_empty());
            let first_day = days_i32[0] as u32;

            let m0 = months[0];

            // Single-day sample: first day with actual journey data
            let sample_date = NaiveDate::from_ymd_opt(target_year, m0, first_day).unwrap();

            // Week: first 7 days of the first data month
            let week_start = NaiveDate::from_ymd_opt(target_year, m0, 1).unwrap();
            let week_end = NaiveDate::from_ymd_opt(target_year, m0, 7).unwrap();

            // Full month: entire first data month
            let month_start = NaiveDate::from_ymd_opt(target_year, m0, 1).unwrap();
            let month_end = last_day_of_month(month_start);

            // Three full months: from Jan 1 of target_year to end of 3rd data month
            let three_months_start = NaiveDate::from_ymd_opt(target_year, 1, 1).unwrap();
            let third_month = months.get(2).copied().unwrap_or(m0);
            let three_months_end =
                last_day_of_month(NaiveDate::from_ymd_opt(target_year, third_month, 1).unwrap());

            // Cross-month: 15th of first month → 14th of (first month + 3)
            let cross_month_start = NaiveDate::from_ymd_opt(target_year, m0, 15).unwrap();
            let end_month_raw = m0 as i32 + 3;
            let (cross_end_year, cross_end_month) = if end_month_raw > 12 {
                (target_year + 1, (end_month_raw - 12) as u32)
            } else {
                (target_year, end_month_raw as u32)
            };
            let cross_month_end =
                NaiveDate::from_ymd_opt(cross_end_year, cross_end_month, 14).unwrap();

            // Full year
            let year_start = NaiveDate::from_ymd_opt(target_year, 1, 1).unwrap();
            let year_end = NaiveDate::from_ymd_opt(target_year, 12, 31).unwrap();

            Ok(DateRanges {
                sample_date,
                week_start,
                week_end,
                month_start,
                month_end,
                three_months_start,
                three_months_end,
                cross_month_start,
                cross_month_end,
                year_start,
                year_end,
            })
        })
        .unwrap()
}

// ===========================================================================
// Benchmark 1: Cold start — clear all cache, then rebuild from scratch
// ===========================================================================

fn bench_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_start");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(60));

    for src in data_sources() {
        let (storage, _t, _d, _s, _c) = setup_storage_with(&src);

        group.bench_function(src.label, |b| {
            b.iter(|| {
                storage.clear_all_cache().unwrap();
                storage
                    .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), false)
                    .unwrap()
            });
        });
    }

    group.finish();
}

// ===========================================================================
// Benchmark 2: Add a trip — DB insert + cache merge action
// ===========================================================================

fn bench_add_trip(c: &mut Criterion) {
    let sample_bitmap = load_sample_bitmap();

    let mut group = c.benchmark_group("add_trip");
    group.sample_size(50);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(15));

    for src in data_sources() {
        let (storage, _t, _d, _s, _c) = setup_storage_with(&src);

        // Warm up: populate the full cache
        storage
            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), false)
            .unwrap();

        let ranges = determine_date_ranges(&storage);

        group.bench_function(src.label, |b| {
            b.iter(|| {
                storage
                    .with_db_txn(|txn| {
                        txn.create_and_insert_journey(
                            ranges.sample_date,
                            None,
                            None,
                            None,
                            JourneyKind::DefaultKind,
                            None,
                            JourneyData::Bitmap(sample_bitmap.clone()),
                        )
                    })
                    .unwrap()
            });
        });
    }

    group.finish();
}

// ===========================================================================
// Benchmark 3: Time machine — get_range_bitmap across 6 window sizes
// ===========================================================================

fn bench_time_machine(c: &mut Criterion) {
    let mut group = c.benchmark_group("time_machine");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));

    for src in data_sources() {
        let (storage, _t, _d, _s, _c) = setup_storage_with(&src);
        let ranges = determine_date_ranges(&storage);

        // Warm up: populate LayerKind::All monthly caches for the full year
        storage
            .get_range_bitmap(ranges.year_start, ranges.year_end, None)
            .unwrap();

        let scenarios: &[(&str, NaiveDate, NaiveDate)] = &[
            ("day", ranges.sample_date, ranges.sample_date),
            ("week_in_month", ranges.week_start, ranges.week_end),
            ("full_month", ranges.month_start, ranges.month_end),
            (
                "3_full_months",
                ranges.three_months_start,
                ranges.three_months_end,
            ),
            (
                "3_months_4_calendar",
                ranges.cross_month_start,
                ranges.cross_month_end,
            ),
            ("full_year", ranges.year_start, ranges.year_end),
        ];

        for &(scenario_label, from, to) in scenarios {
            group.bench_function(BenchmarkId::new(src.label, scenario_label), |b| {
                b.iter(|| storage.get_range_bitmap(from, to, None).unwrap());
            });
        }
    }

    group.finish();
}

// ===========================================================================
// Benchmark 4: Delete / update — cache invalidation cost
// ===========================================================================

fn bench_delete_update(c: &mut Criterion) {
    let sample_bitmap = load_sample_bitmap();

    let mut group = c.benchmark_group("delete_update");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));

    for src in data_sources() {
        let (storage, _t, _d, _s, _c) = setup_storage_with(&src);

        // Warm up: populate the full cache
        storage
            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), false)
            .unwrap();

        let ranges = determine_date_ranges(&storage);
        // alt_date differs from sample_date → triggers Invalidate
        let alt_date = ranges.sample_date + chrono::Duration::days(1);

        // --- delete_journey ---
        group.bench_function(BenchmarkId::new(src.label, "delete_journey"), |b| {
            b.iter_batched(
                || {
                    // Setup (not measured): insert a journey, return its id
                    storage
                        .with_db_txn(|txn| {
                            txn.create_and_insert_journey(
                                ranges.sample_date,
                                None,
                                None,
                                None,
                                JourneyKind::DefaultKind,
                                None,
                                JourneyData::Bitmap(sample_bitmap.clone()),
                            )
                        })
                        .unwrap()
                },
                |id| {
                    // Measured: invalidate cache (delete) + trigger full rebuild
                    storage.with_db_txn(|txn| txn.delete_journey(&id)).unwrap();
                    storage
                        .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), false)
                        .unwrap()
                },
                BatchSize::SmallInput,
            );
        });

        // --- update_journey_metadata ---
        group.bench_function(
            BenchmarkId::new(src.label, "update_journey_metadata"),
            |b| {
                b.iter_batched(
                    || {
                        // Setup (not measured): insert a journey with sample_date
                        storage
                            .with_db_txn(|txn| {
                                txn.create_and_insert_journey(
                                    ranges.sample_date,
                                    None,
                                    None,
                                    None,
                                    JourneyKind::DefaultKind,
                                    None,
                                    JourneyData::Bitmap(sample_bitmap.clone()),
                                )
                            })
                            .unwrap()
                    },
                    |id| {
                        // Measured: invalidate cache (update) + trigger full rebuild
                        storage
                            .with_db_txn(|txn| {
                                txn.update_journey_metadata(
                                    &id,
                                    alt_date,
                                    None,
                                    None,
                                    None,
                                    JourneyKind::DefaultKind,
                                )
                            })
                            .unwrap();
                        storage
                            .get_latest_bitmap_for_main_map_renderer(&Some(LayerKind::All), false)
                            .unwrap()
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    cache_benches,
    bench_cold_start,
    bench_add_trip,
    bench_time_machine,
    bench_delete_update,
);
criterion_main!(cache_benches);
