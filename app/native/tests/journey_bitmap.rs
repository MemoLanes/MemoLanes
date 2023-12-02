use native::{journey_bitmap::JourneyBitmap, map_renderer::{MapRenderer, RenderResult}};
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

#[test]
fn merge() {
    let start_lng = 151.1435370795134;
    let start_lat = -33.793291910360125;
    let end_lng = 151.2783692841415;
    let end_lat = -33.943600147192235;

    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut other_journey_bitmap = JourneyBitmap::new();
    other_journey_bitmap.add_line(start_lng, end_lat, end_lng, start_lat);

    journey_bitmap.merge(other_journey_bitmap);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(12.0, start_lng, start_lat, end_lng, end_lat)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_merge",
        "cd60e35e3fce1c113b10ca2635eacd658ff225be",
    );
}

#[test]
fn difference(){
    let start_lng = 151.1435370795134;
    let start_lat = -33.793291910360125;
    let end_lng = 151.2783692841415;
    let end_lat = -33.943600147192235;

    let mut journey_bitmap:JourneyBitmap=JourneyBitmap::new();
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut other_jb:JourneyBitmap=JourneyBitmap::new();
    other_jb.add_line(start_lng, end_lat, end_lng, start_lat);

    journey_bitmap.difference(other_jb);

    let mut map_renderer:MapRenderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(12.0, start_lng, start_lat, end_lng, end_lat)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_difference",
        "1371a1b757bf3cbf7ac973e26eb7685917a7942d",
    );
}

#[test]
fn intersection(){
    let start_lng = 151.1435370795134;
    let start_lat = -33.793291910360125;
    let end_lng = 151.2783692841415;
    let end_lat = -33.943600147192235;

    let mut journey_bitmap:JourneyBitmap=JourneyBitmap::new();
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let mut other_jb:JourneyBitmap=JourneyBitmap::new();
    other_jb.add_line(start_lng, end_lat, end_lng, start_lat);

    journey_bitmap.intersection(other_jb);

    let mut map_renderer:MapRenderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(12.0, start_lng, start_lat, end_lng, end_lat)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_intersection",
        "1371a1b757bf3cbf7ac973e26eb7685917a7942d",
    );
}