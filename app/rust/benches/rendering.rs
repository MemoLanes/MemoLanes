use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use memolanes_core::import_data;
use memolanes_core::renderer::map_renderer;
use memolanes_core::utils::lng_lat_to_tile_x_y;

fn tile_buffer_creation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("tile_buffer_creation");
    group.sample_size(10); // Lower sample size as this is more expensive

    let (bitmap_data, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_3.zip").unwrap();

    // Shenzhen universiade
    let lng = 114.212470;
    let lat = 22.697006;

    let zoom_levels: Vec<i16> = (3..=15).step_by(2).collect(); // 5, 7, 9, 11, 13, 15
    let width = 2;
    let height = 2;
    let buffer_size_powers = vec![8, 9, 10]; // 256px, 512px, 1024px tiles

    // Benchmark TileBuffer::from_journey_bitmap
    for zoom in &zoom_levels {
        let (tile_x, tile_y) = lng_lat_to_tile_x_y(lng, lat, *zoom as i32);
        let (tile_x, tile_y) = (tile_x as i64, tile_y as i64);

        for buffer_size_power in &buffer_size_powers {
            group.bench_with_input(
                BenchmarkId::new(
                    "tile_buffer_creation",
                    format!(
                        "z{:02}_res{:04}_x{}_y{}_{width}x{height}",
                        zoom,
                        1 << buffer_size_power,
                        tile_x,
                        tile_y
                    ),
                ),
                &(zoom, width, height, tile_x, tile_y, buffer_size_power),
                |b, (zoom, width, height, tile_x, tile_y, buffer_size_power)| {
                    b.iter(|| {
                        std::hint::black_box(
                            map_renderer::tile_buffer_from_journey_bitmap(
                                &bitmap_data,
                                *tile_x, // use calculated x coordinate
                                *tile_y, // use calculated y coordinate
                                **zoom,
                                *width,
                                *height,
                                **buffer_size_power,
                            )
                            .unwrap(),
                        )
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(rendering_benches, tile_buffer_creation_benchmarks);
criterion_main!(rendering_benches);
