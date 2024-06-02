use criterion::{criterion_group, criterion_main, Criterion};

use memolanes_core::{journey_bitmap::JourneyBitmap, map_renderer::*};

#[test]
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    let start_lng = 151.1435370795134;
    let start_lat = -33.793291910360125;
    let end_lng = 151.2783692841415;
    let end_lat = -33.943600147192235;
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        map_renderer.maybe_render_map_overlay(11, start_lng, start_lat, end_lng, end_lat);
    let render_result = render_result.unwrap();
}

fn map_renderer(c: &mut Criterion) {
    c.bench_function("map_renderer", |b| {
        let mut journey_bitmap = JourneyBitmap::new();
        let start_lng = 151.1435370795134;
        let start_lat = -33.793291910360125;
        let end_lng = 151.2783692841415;
        let end_lat = -33.943600147192235;
        journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);
        let mut map_renderer = MapRenderer::new(journey_bitmap);

        let zoom = 11;

        b.iter(|| {
            std::hint::black_box({
                map_renderer.reset();
                let render_result = map_renderer
                    .maybe_render_map_overlay(zoom, start_lng, start_lat, end_lng, end_lat);
                render_result.unwrap();
            });
        });
    });
}

criterion_group!(benches, map_renderer,);
criterion_main!(benches);
