use native::{journey_bitmap::JourneyBitmap, map_renderer::MapRenderer};
mod test_utils;

#[test]
fn add_line_cross_antimeridian() {
    let mut journey_bitmap = JourneyBitmap::new();

    // Melbourne to Hawaii
    let (start_lng, start_lat, end_lng, end_lat) =
        (144.847737, 37.6721702, -160.3644029, 21.3186185);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    // Hawaii to Guan
    let (start_lng, start_lat, end_lng, end_lat) =
        (-160.3644029, 21.3186185, 121.4708788, 9.4963078);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(0.0, -170.0, 80.0, 170.0, -80.0)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_add_line_cross_antimeridian",
        "3eb61d8bae656e73894b54c1cd009046caf6f75f",
    );
}
