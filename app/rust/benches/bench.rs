use criterion::{criterion_group, criterion_main, Criterion};
use memolanes_core::{
    import_data, journey_area_utils, journey_bitmap::JourneyBitmap, merged_journey_builder,
};
use std::collections::HashMap;

fn journey_area_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("area_calculation");
    group.sample_size(10);

    group.bench_function("compute_journey_bitmap_area: simple", |b| {
        let (bitmap_import, _warnings) =
            import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();
        let mut dummy_map: HashMap<(u16, u16), f64> = HashMap::new();
        b.iter(|| {
            dummy_map.clear();
            std::hint::black_box(journey_area_utils::compute_journey_bitmap_area(
                &bitmap_import,
                &mut dummy_map,
            ))
        })
    });

    group.bench_function(
        "compute_journey_bitmap_area: nelson_to_wharariki_beach",
        |b| {
            let raw_data =
                import_data::load_gpx("./tests/data/nelson_to_wharariki_beach.gpx").unwrap();

            let journey_vector =
                import_data::journey_vector_from_raw_data(raw_data, false).unwrap();
            let mut journey_bitmap = JourneyBitmap::new();
            merged_journey_builder::add_journey_vector_to_journey_bitmap(
                &mut journey_bitmap,
                &journey_vector,
            );
            let mut dummy_map: HashMap<(u16, u16), f64> = HashMap::new();
            b.iter(|| {
                dummy_map.clear();
                std::hint::black_box(journey_area_utils::compute_journey_bitmap_area(
                    &journey_bitmap,
                    &mut dummy_map,
                ))
            })
        },
    );

    group.finish();
}

criterion_group!(benches, journey_area_calculation);
criterion_main!(benches);
