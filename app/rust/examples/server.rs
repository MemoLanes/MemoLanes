use ctrlc;
use journey_kernel::journey_bitmap::JourneyBitmap;
use memolanes_core::api::api::CameraOption;
use memolanes_core::renderer::MapServer;
use rand::Rng;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut server = MapServer::new();
    server
        .start("localhost", 0)
        .expect("Failed to start server");

    println!(
        "see demo track at: {}#lng=114.05&lat=22.54&zoom=12",
        server.get_url()
    );

    std::thread::sleep(Duration::from_millis(200));

    // demo for a dynamic map
    let journey_bitmap2 = JourneyBitmap::new();
    let arc_journey_bitmap2 = Arc::new(Mutex::new(journey_bitmap2));
    server.set_journey_bitmap_with_poll_handler(Arc::downgrade(&arc_journey_bitmap2), None);

    // Set initial camera position for China region
    server.set_provisioned_camera_option(Some(CameraOption {
        lng: 106.5,
        lat: 30.0,
        zoom: 4.0,
    }));

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
            server.set_provisioned_camera_option(Some(CameraOption {
                lng,
                lat,
                zoom: 12.0, // Closer zoom to see the path detail
            }));

            // Notify clients that the bitmap needs reloading
            server.set_needs_reload();

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
