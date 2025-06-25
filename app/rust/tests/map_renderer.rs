pub mod test_utils;
use memolanes_core::{journey_bitmap::JourneyBitmap, renderer::*};

#[macro_use]
extern crate assert_float_eq;

#[test]
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    let start_lng = 151.1435370795134;
    let start_lat = -33.793291910360125;
    let end_lng = 151.2783692841415;
    let end_lat = -33.943600147192235;
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 11, start_lng, start_lat, end_lng, end_lat);
    assert_f64_near!(render_result.left, 150.8203125);
    assert_f64_near!(render_result.top, -33.578014746143985);
    assert_f64_near!(render_result.right, 151.5234375);
    assert_f64_near!(render_result.bottom, -34.16181816123038);

    test_utils::verify_image("map_renderer_basic", &render_result.data);
}
