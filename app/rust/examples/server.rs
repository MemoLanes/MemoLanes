use ctrlc;
use journey_kernel::journey_bitmap::JourneyBitmap;
use memolanes_core::api::api::CameraOption;
use memolanes_core::renderer::get_default_camera_option_from_journey_bitmap;
use memolanes_core::renderer::MapServer;
use rand::Rng;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
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

    // demo for a static map
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);

    let default_camera_option = get_default_camera_option_from_journey_bitmap(&journey_bitmap);

    let arc_journey_bitmap = Arc::new(Mutex::new(journey_bitmap));
    let token = server.register(Arc::downgrade(&arc_journey_bitmap));
    if let Some(ref default_camera_option) = default_camera_option {
        token.set_provisioned_camera_option(default_camera_option.clone());
    }

    println!("view static map at: {}", token.url());

    // demo for a dynamic map
    let journey_bitmap2 = JourneyBitmap::new();
    let arc_journey_bitmap2 = Arc::new(Mutex::new(journey_bitmap2));
    let token2 = server.register(Arc::downgrade(&arc_journey_bitmap2));

    // Set initial camera position for China region
    token2.set_provisioned_camera_option(CameraOption {
        lng: 106.5,
        lat: 30.0,
        zoom: 4.0,
    });

    println!("view dynamic map at: {}", token2.url());

    // Spawn a thread for dynamic line addition
    let arc_journey_bitmap2_clone = arc_journey_bitmap2.clone();
    std::thread::spawn(move || {
        let mut rng = rand::thread_rng();
        let mut lat = 30.0;
        let mut lng = 106.5;
        let lng_step = 0.00252;
        let lat_step = lng_step;

        loop {
            // Add a new line segment
            {
                let mut bitmap = arc_journey_bitmap2_clone.lock().unwrap();
                let next_lng = lng + lng_step;
                let next_lat = lat + rng.gen_range(-lat_step..=lat_step);
                bitmap.add_line(lng, lat, next_lng, next_lat);
                lng = next_lng;
                lat = next_lat;
            }

            // Update camera to follow the latest point
            token2.set_provisioned_camera_option(CameraOption {
                lng,
                lat,
                zoom: 12.0, // Closer zoom to see the path detail
            });

            // Notify clients that the bitmap needs reloading
            token2.set_needs_reload();

            // Sleep for 200ms
            std::thread::sleep(Duration::from_millis(200));
        }
    });

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
