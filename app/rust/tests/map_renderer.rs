pub mod test_utils;

use memolanes_core::blur::gaussian_blur;
use memolanes_core::import_data;
use memolanes_core::tile_renderer::TileRenderer;
use memolanes_core::{journey_bitmap::JourneyBitmap, map_renderer::*};
use tiny_skia;
// use fastblur::gaussian_blur;

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
        map_renderer.maybe_render_map_overlay(11.0, start_lng, start_lat, end_lng, end_lat);
    let render_result = render_result.unwrap();
    assert_eq!(render_result.left, 150.99609375);
    assert_eq!(render_result.top, -33.72433966174759);
    assert_eq!(render_result.right, 151.34765625);
    assert_eq!(render_result.bottom, -34.016241889667015);

    test_utils::assert_image(
        &render_result.data,
        "map_renderer_basic",
        "2f55c28e9757b76d9b20efc600127eac9b3432f2",
    );

    // a small move shouldn't trigger a re-render
    let render_result = map_renderer.maybe_render_map_overlay(
        11.0,
        151.143537079,
        -33.79329191036,
        151.278369284,
        -33.94360014719,
    );
    assert!(render_result.is_none());

    // but a bigger move will
    let render_result = map_renderer.maybe_render_map_overlay(11.0, 151.0, -33.0, 151.0, -33.0);
    assert!(render_result.is_some());
}

#[test]
fn blurred_rendering() {
    let (journey_bitmap, _warnings) =
        import_data::load_fow_sync_data("./tests/data/fow_1.zip").unwrap();

    let mut tile_renderer = TileRenderer::new();
    tile_renderer.set_bg_color(tiny_skia::Color::from_rgba8(245, 240, 229, 255));
    // tile_renderer.set_bg_color(tiny_skia::Color::from_rgba8(245, 50, 229, 255));
    tile_renderer.set_fg_color(tiny_skia::Color::from_rgba8(110, 0, 0, 255));
    // tile_renderer.set_fg_color(tiny_skia::Color::from_rgba8(110, 190, 80, 255));
    let mut map_renderer = MapRenderer::new_with_tile_renderer(journey_bitmap, tile_renderer);

    let left_lng = 109.369177;
    let bottom_lat = 18.227972;
    let right_lng = 109.680797;
    let top_lat = 18.410264;

    let render_result =
        map_renderer.maybe_render_map_overlay(11.0, left_lng, top_lat, right_lng, bottom_lat);
    let render_result = render_result.unwrap();

    test_utils::assert_image(
        &render_result.data,
        "map_render_better_graphics",
        "027a75d45416cdc470d9f7c3fd2660a50f89aa45",
    );

    let mut pixmap = tiny_skia::Pixmap::decode_png(&render_result.data).unwrap();
    let pixmap_data = pixmap.data_mut();

    gaussian_blur(
        pixmap_data,
        render_result.width.try_into().unwrap(),
        render_result.height.try_into().unwrap(),
        2.0,
    );

    // let pixmap_blurred = tiny_skia::Pixmap::from_vec(
    //     pixmap_data.to_vec(),
    //     tiny_skia::IntSize::from_wh(render_result.width, render_result.height).unwrap()
    // ).unwrap();
    let image_blurred = pixmap.encode_png().unwrap();

    test_utils::assert_image(
        &image_blurred,
        "map_render_blurred",
        "a094374b53efc13d475c97d20f2796b3cc4d1969",
    );

    // let mut rgb = Vec::with_capacity(pixmap_data.len()/4);
    // let mut a = Vec::with_capacity(pixmap_data.len()/4);

    // for chunk in pixmap_data.chunks(4) {
    //     rgb.push([chunk[0], chunk[1], chunk[2]]);
    //     a.push(chunk[3]);
    // }

    // gaussian_blur(
    //     &mut rgb,
    //     render_result.width.try_into().unwrap(),
    //     render_result.height.try_into().unwrap(),
    //     2.0
    // );

    // let mut combined = Vec::with_capacity(pixmap_data.len());

    // for i in 0..a.len() {
    //     combined.push(rgb[i][0]);
    //     combined.push(rgb[i][1]);
    //     combined.push(rgb[i][2]);
    //     combined.push(a[i]);
    // }

    // let pixmap_blurred = tiny_skia::Pixmap::from_vec(
    //     combined,
    //     tiny_skia::IntSize::from_wh(render_result.width, render_result.height).unwrap()
    // ).unwrap();
    // let image_blurred = pixmap_blurred.encode_png().unwrap();

    // test_utils::assert_image(
    //     &image_blurred,
    //     "map_render_blurred",
    //     "a094374b53efc13d475c97d20f2796b3cc4d1969",
    // );
}
