use ctrlc;
use journey_kernel::journey_bitmap::JourneyBitmap;
use memolanes_core::api::api::get_default_camera_option_from_journey_bitmap;
use memolanes_core::renderer::MapServer;
use std::sync::Arc;
use std::sync::Mutex;
const START_LNG: f64 = 151.14;
const START_LAT: f64 = -33.79;
const END_LNG: f64 = 141.27;
const END_LAT: f64 = -25.94;
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

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = MapServer::new("localhost", 0);
    server.start().expect("Failed to start server");

    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);

    let default_camera_option = get_default_camera_option_from_journey_bitmap(&journey_bitmap);

    let arc_journey_bitmap = Arc::new(Mutex::new(journey_bitmap));
    let token = server.register(Arc::downgrade(&arc_journey_bitmap));
    println!("token: {}", token.url());

    if let Some(ref default_camera_option) = default_camera_option {
        token.set_provisioned_camera_option(default_camera_option.clone());
    }

    // Set up ctrl+c handler
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C! Shutting down...");
        std::process::exit(0);
    })?;

    // Block the main thread to keep server running
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
