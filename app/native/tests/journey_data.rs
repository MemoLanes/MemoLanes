pub mod test_utils;

use std::fs::File;
use native::{journey_bitmap::JourneyBitmap,journey_data::JourneyData};

const DATA_FILE_PATH:&str ="./tests/for_inspection/data.dat";


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

fn get_journey_bitmap()->JourneyBitmap{
    let mut journey_bitmap = JourneyBitmap::new();

    // // Melbourne to Hawaii
    // let (start_lng, start_lat, end_lng, end_lat) =
    //     (144.847737, 37.6721702, -160.3644029, 21.3186185);
    // journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);

    // // Hawaii to Guan
    // let (start_lng, start_lat, end_lng, end_lat) =
    //     (-160.3644029, 21.3186185, 121.4708788, 9.4963078);
    // journey_bitmap.add_line(start_lng, start_lat, end_lng, end_lat);
    
    draw_line1(&mut journey_bitmap);
    // draw_line2(&mut journey_bitmap);

    journey_bitmap
}

#[test]
fn serilize_journey_data_bitmap(){      
    let f=File::create(DATA_FILE_PATH).unwrap();
    let _=JourneyData::Bitmap(get_journey_bitmap()).serialize(f);  
}

#[test]
fn deserilize_journey_data_bitmap(){
    let reader_result = File::open(DATA_FILE_PATH);
    match reader_result {
        Ok(reader)=>{
            let result = JourneyData::deserialize(reader, native::journey_header::JourneyType::Bitmap);
            match result {
                Ok(journey_data)=>{
                    let origin_journey_bitmap=get_journey_bitmap();
                    if let JourneyData::Bitmap(journey_bitmap)=journey_data{
                        assert_eq!(journey_bitmap,origin_journey_bitmap);
                    }
                },
                Err(e)=>{
                    println!("Error: {}", e);
                }
            }    
        },
        Err(e)=>{
            println!("File Open Error: {}", e);
        }
    }

    
}