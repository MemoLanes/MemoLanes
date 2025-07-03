pub mod test_utils;
use memolanes_core::{
    import_data, journey_area_utils, journey_bitmap::JourneyBitmap, journey_data::JourneyData,
    journey_header::JourneyType, merged_journey_builder, renderer::MapRenderer,
};

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

    let (start_lng, start_lat, end_lng, end_lat) =
        (175.4708788, 5.4963078, -175.3644029, -28.3186185);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let (start_lng, start_lat, end_lng, end_lat) =
        (-175.3644029, -28.3186185, 175.4708788, -49.4963078);
    journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 0, -170.0, 80.0, 170.0, -80.0);
    test_utils::verify_image(
        "journey_bitmap_add_line_cross_antimeridian",
        &render_result.data,
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
fn basic() {
    let mut journey_bitmap = JourneyBitmap::new();
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, START_LAT);
    journey_bitmap.add_line(END_LNG, END_LAT, START_LNG, END_LAT);
    journey_bitmap.add_line(START_LNG, START_LAT, START_LNG, END_LAT);
    journey_bitmap.add_line(END_LNG, END_LAT, END_LNG, START_LAT);

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 12, START_LNG, START_LAT, END_LNG, END_LAT);
    test_utils::verify_image("journey_bitmap_basic", &render_result.data);
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

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 12, START_LNG, START_LAT, END_LNG, END_LAT);
    test_utils::verify_image("journey_bitmap_merge_with_render", &render_result.data);
}

#[test]
fn intersection_and_difference() {
    // line3 - (line3 - line4) = line3 & line4

    let mut rhs = JourneyBitmap::new();
    draw_line3(&mut rhs);
    {
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        rhs.intersection(&line4);
    }

    let mut lhs = JourneyBitmap::new();
    draw_line3(&mut lhs);
    {
        let mut line3 = JourneyBitmap::new();
        draw_line3(&mut line3);
        let mut line4 = JourneyBitmap::new();
        draw_line4(&mut line4);
        line3.difference(&line4);
        lhs.difference(&line3);
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
        journey_bitmap.difference(&line1);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
    {
        // line1 & line2
        let mut journey_bitmap = JourneyBitmap::new();
        draw_line1(&mut journey_bitmap);
        let mut line2 = JourneyBitmap::new();
        draw_line2(&mut line2);
        journey_bitmap.intersection(&line2);
        assert_eq!(journey_bitmap, JourneyBitmap::new());
    }
}

#[test]
fn serialization() {
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    let journey_data = JourneyData::Bitmap(journey_bitmap);

    let mut buf = Vec::new();
    journey_data.serialize(&mut buf).unwrap();

    println!("size: {}", buf.len());

    let journey_data_roundtrip =
        JourneyData::deserialize(buf.as_slice(), JourneyType::Bitmap).unwrap();
    assert_eq!(journey_data, journey_data_roundtrip);
}

fn vector_to_bitmap(name: &str, zoom: i32, filename_override: Option<&str>) {
    let filename = match filename_override {
        None => format!("./tests/data/raw_gps_{}.gpx", name),
        Some(filename) => format!("./tests/data/{}", filename),
    };
    let loaded_data = import_data::load_gpx(&filename).unwrap();
    let journey_vector = import_data::journey_vector_from_raw_data(loaded_data, true).unwrap();
    let mut journey_bitmap = JourneyBitmap::new();
    merged_journey_builder::add_journey_vector_to_journey_bitmap(
        &mut journey_bitmap,
        &journey_vector,
    );

    // compute the bounding box
    let (mut left, mut right, mut top, mut bottom): (f64, f64, f64, f64) = (180., -180., -90., 90.);
    for segment in &journey_vector.track_segments {
        for point in &segment.track_points {
            left = left.min(point.longitude);
            right = right.max(point.longitude);
            top = top.max(point.latitude);
            bottom = bottom.min(point.latitude);
        }
    }

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, zoom, left, top, right, bottom);
    test_utils::verify_image(
        &format!("journey_bitmap_vector_to_bitmap_{}", name),
        &render_result.data,
    );
}

// `raw_gps_shanghai.gpx` is already covered by the end to end test.

#[test]
fn vector_to_bitmap_shenzhen_stationary() {
    vector_to_bitmap("shenzhen_stationary", 16, None);
}

#[test]
fn vector_to_bitmap_laojunshan() {
    vector_to_bitmap("laojunshan", 16, None);
}

#[test]
fn vector_to_bitmap_nelson_to_wharariki_beach() {
    // `nelson_to_wharariki_beach.gps` is not a raw gps because it does not contian timestamps,
    // but it is good enough for this test.
    vector_to_bitmap(
        "nelson_to_wharariki_beach",
        9,
        Some("nelson_to_wharariki_beach.gpx"),
    );
}

#[test]
fn draw_single_point() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.add_line(120.0, 30.0, 120.0, 30.0);

    assert_eq!(
        journey_area_utils::compute_journey_bitmap_area(&journey_bitmap, None),
        68
    );
}

#[test]
fn draw_line_with_width2() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.add_line(120.0, 60.0, 120.0, 60.005);
    journey_bitmap.add_line(120.0, 60.005, 120.01, 60.01);
    journey_bitmap.add_line(120.01, 60.01, 120.0, 60.0);
    journey_bitmap.add_line(120.0, 60.0, 120.005, 60.0);

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 13, 120.0, 60.01, 120.01, 60.0);
    test_utils::verify_image("draw_line_with_width2", &render_result.data);
}

#[test]
fn draw_line_with_width3() {
    let mut journey_bitmap = JourneyBitmap::new();

    journey_bitmap.add_line(120.0, 70.0, 120.0, 70.005);
    journey_bitmap.add_line(120.0, 70.005, 120.01, 70.01);
    journey_bitmap.add_line(120.01, 70.01, 120.0, 70.0);
    journey_bitmap.add_line(120.0, 70.0, 120.005, 70.0);

    let map_renderer = MapRenderer::new(journey_bitmap);

    let render_result =
        test_utils::render_map_overlay(&map_renderer, 13, 120.0, 70.01, 120.01, 70.0);
    test_utils::verify_image("draw_line_with_width3", &render_result.data);
}
