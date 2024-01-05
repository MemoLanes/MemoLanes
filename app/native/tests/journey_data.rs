pub mod test_utils;

use native::{
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};
use std::io::Cursor;

const START_LNG: f64 = 151.1435370795134;
const START_LAT: f64 = -33.793291910360125;
const END_LNG: f64 = 151.2783692841415;
const END_LAT: f64 = -33.943600147192235;

#[warn(dead_code)]
fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
#[warn(dead_code)]
fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap.add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}

fn get_journey_bitmap() -> JourneyBitmap {
    let mut journey_bitmap = JourneyBitmap::new();

    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);

    journey_bitmap
}

#[test]
fn serilize_deserilize_journey_data_bitmap() {
    let mut buf: Vec<u8> = Vec::new();
    let writer_cursor = Cursor::new(&mut buf);
    let _ = JourneyData::Bitmap(get_journey_bitmap()).serialize(writer_cursor);
    println!("bitmap serilize success");

    let reader_cursor = Cursor::new(buf);
    let result =
        JourneyData::deserialize(reader_cursor, native::journey_header::JourneyType::Bitmap);
    match result {
        Ok(journey_data) => {
            let origin_journey_bitmap = get_journey_bitmap();
            if let JourneyData::Bitmap(journey_bitmap) = journey_data {
                assert_eq!(journey_bitmap, origin_journey_bitmap);
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

fn get_journey_vector() -> JourneyVector {
    let track_point1 = TrackPoint {
        latitude: START_LAT,
        longitude: START_LNG,
    };
    let track_point2 = TrackPoint {
        latitude: END_LAT,
        longitude: END_LNG,
    };
    let track_segment = TrackSegment {
        track_points: vec![track_point1, track_point2],
    };
    JourneyVector {
        track_segments: vec![track_segment],
    }
}

#[test]
fn serilize_deserialize_journey_data_vector() {
    let mut buf: Vec<u8> = Vec::new();
    let writer_cursor = Cursor::new(&mut buf);
    let _ = JourneyData::Vector(get_journey_vector()).serialize(writer_cursor);
    println!("vector serilize success");

    let reader_cursor = Cursor::new(buf);
    let result =
        JourneyData::deserialize(reader_cursor, native::journey_header::JourneyType::Vector);
    match result {
        Ok(journey_data) => {
            let origin_journey_vector = get_journey_vector();
            if let JourneyData::Vector(journey_vector) = journey_data {
                assert_eq!(journey_vector, origin_journey_vector);
            }
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
