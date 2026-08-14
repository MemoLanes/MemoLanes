pub mod test_utils;
use memolanes_core::{journey_bitmap::JourneyBitmap, renderer::*};

#[macro_use]
extern crate assert_float_eq;

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;

#[test]
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = test_utils::render_map_overlay(
        &mut map_renderer,
        11,
        START_LNG,
        START_LAT,
        END_LNG,
        END_LAT,
    );
    assert_f64_near!(render_result.left, 150.8203125);
    assert_f64_near!(render_result.top, -33.578014746143985);
    assert_f64_near!(render_result.right, 151.5234375);
    assert_f64_near!(render_result.bottom, -34.16181816123038);

    test_utils::verify_image("map_renderer_basic", &render_result.data);
}

#[test]
fn no_op_update_preserves_version_and_real_update_invalidates_it() {
    let mut renderer = MapRenderer::new(JourneyBitmap::new());
    renderer.update(|bitmap, changed| {
        bitmap.add_line_with_change_callback(START_LNG, START_LAT, END_LNG, END_LAT, changed);
    });
    let version = renderer.get_current_version();
    let version_string = renderer.get_version_string();
    let area = renderer.get_current_area();

    renderer.update(|bitmap, changed| {
        bitmap.add_line_with_change_callback(START_LNG, START_LAT, END_LNG, END_LAT, changed);
    });

    assert_eq!(renderer.get_current_version(), version);
    assert!(renderer
        .get_latest_bitmap_if_changed(Some(&version_string))
        .is_none());
    assert_eq!(renderer.get_current_area(), area);

    renderer.update(|bitmap, changed| {
        bitmap.add_line_with_change_callback(
            START_LNG + 1.0,
            START_LAT,
            END_LNG + 1.0,
            END_LAT,
            changed,
        );
    });

    assert_eq!(renderer.get_current_version(), version.wrapping_add(1));
    assert!(renderer
        .get_latest_bitmap_if_changed(Some(&version_string))
        .is_some());
    assert!(renderer.get_current_area() > area);
}
