pub mod test_utils;

use native::{journey_bitmap::JourneyBitmap, map_renderer::MapRenderer};

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

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;
const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}
fn draw_line3(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(MID_LNG, START_LAT, MID_LNG, END_LAT)
}
fn draw_line4(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, MID_LAT, END_LNG, MID_LAT)
}

#[test]
fn merge_with_render() {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);

    let mut other_journey_bitmap = JourneyBitmap::new();
    draw_line2(&mut other_journey_bitmap);

    journey_bitmap.merge(other_journey_bitmap);

    let mut sample_journaey_bitmap = JourneyBitmap::new();
    draw_line1(&mut sample_journaey_bitmap);
    draw_line2(&mut sample_journaey_bitmap);
    assert_eq!(journey_bitmap, sample_journaey_bitmap);

    let mut map_renderer = MapRenderer::new(journey_bitmap);

    let render_result = map_renderer
        .maybe_render_map_overlay(12.0, START_LNG, START_LAT, END_LNG, END_LAT)
        .unwrap();
    test_utils::assert_image(
        &render_result.data.0,
        "journey_bitmap_merge_with_render",
        "cd60e35e3fce1c113b10ca2635eacd658ff225be",
    );
}

#[test]
fn intersection_and_difference() {
    // line3 - (line3 - line4) = line3 & line4

    let mut rhs = JourneyBitmap::new();
    draw_line3(&mut rhs);
    {
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        rhs.intersection(line4);
    }

    let mut lhs = JourneyBitmap::new();
    draw_line3(&mut lhs);
    {
        let mut line3 = JourneyBitmap::new();
        draw_line3(&mut line3);
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        line3.difference(line4);
        lhs.difference(line3);
    }

    assert_ne!(lhs, JourneyBitmap::new());
    assert_eq!(lhs, rhs);
}

#[test]
fn intersection_and_difference_produce_empty() {
    {
        // line1 - line1
        let mut journey_bitmap = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap);
        let mut line1 = JourneyBitmap::new();
        draw_line1(&mut line1);
        journey_bitmap.difference(line1);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
    {
        // line1 & line2
        let mut journey_bitmap = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap);
        let mut line2 = JourneyBitmap::new();
        draw_line2(&mut line2);
        journey_bitmap.intersection(line2);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
}
