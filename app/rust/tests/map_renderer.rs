pub mod test_utils;
use memolanes_core::{journey_bitmap::JourneyBitmap, map_renderer::*};

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

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        map_renderer.maybe_render_map_overlay(11, start_lng, start_lat, end_lng, end_lat);
    let render_result = render_result.unwrap();
    assert_f64_near!(render_result.left, 150.8203125);
    assert_f64_near!(render_result.top, -33.578014746143985);
    assert_f64_near!(render_result.right, 151.5234375);
    assert_f64_near!(render_result.bottom, -34.16181816123038);

    test_utils::assert_image(
        &render_result.data,
        "map_renderer_basic",
        "df0ef4aa3953cbe503babb56f133d7d11eecba6e",
    );

    // a small move shouldn't trigger a re-render
    let render_result = map_renderer.maybe_render_map_overlay(
        11,
        151.143537079,
        -33.79329191036,
        151.278369284,
        -33.94360014719,
    );
    assert!(render_result.is_none());

    // but a bigger move will
    let render_result = map_renderer.maybe_render_map_overlay(11, 151.0, -33.0, 151.0, -33.0);
    assert!(render_result.is_some());
}
