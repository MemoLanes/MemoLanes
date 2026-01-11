use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::renderer::MapRenderer;
mod shared;
use shared::MapServer;

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
    // Initialize the logger with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    println!("############ BREAK CHANGE NOTICE ###############");
    println!("Starting from Dec 2025, user need to hosting the static resource by themselves and the rust side will only handle the dynamic requests.");
    println!("Please make sure you have `yarn dev` running in journey_kernel folder.");
    println!(
        "- By default, we assume the static resources are available at `http://localhost:8080`"
    );
    println!(
        "- You may also pass the static resources url via the DEV_SERVER environment variable"
    );
    println!("    eg: `DEV_SERVER=http://localhost:8080 cargo run --release --example server -- --nocapture`");
    println!("################################################");

    // ========== Server 1: Simple Map (static with crossed lines) ==========
    let registry_simple = Arc::new(Mutex::new(None));
    let server_simple =
        MapServer::create_and_start_with_registry("localhost", None, registry_simple.clone())
            .expect("Failed to start simple map server");

    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);

    let map_renderer_static = MapRenderer::new(journey_bitmap);
    server_simple.set_map_renderer(Arc::new(Mutex::new(map_renderer_static)));

    println!("================================================");
    println!(
        "[Simple Map Server]:   {}",
        server_simple.get_http_url()
    );
    println!(
        "[Simple Map Local]:    {}",
        server_simple.get_file_url()
    );

    // ========== Server 2: Medium Map (loaded from fow_3.zip) ==========
    let registry_medium = Arc::new(Mutex::new(None));
    let server_medium =
        MapServer::create_and_start_with_registry("localhost", None, registry_medium.clone())
            .expect("Failed to start medium map server");

    let (joruney_bitmap_fow, _) =
        import_data::load_fow_sync_data("./tests/data/fow_3.zip").unwrap();
    let map_renderer_fow = MapRenderer::new(joruney_bitmap_fow);
    server_medium.set_map_renderer(Arc::new(Mutex::new(map_renderer_fow)));

    println!(
        "[Medium Map Server]:   {}",
        server_medium.get_http_url()
    );

    // ========== Server 3: Dynamic Map (randomly drawn lines) ==========
    let registry_dynamic = Arc::new(Mutex::new(None));
    let server_dynamic =
        MapServer::create_and_start_with_registry("localhost", None, registry_dynamic.clone())
            .expect("Failed to start dynamic map server");

    let journey_bitmap2 = JourneyBitmap::new();
    let map_renderer = MapRenderer::new(journey_bitmap2);
    let map_renderer_arc = Arc::new(Mutex::new(map_renderer));
    let map_renderer_arc_clone = map_renderer_arc.clone();
    server_dynamic.set_map_renderer(map_renderer_arc);

    println!(
        "[Dynamic Map Server]:  {}",
        server_dynamic.get_http_url()
    );

    // Wrap servers in Arc<Mutex<>> for thread-safe access
    let server_simple = Arc::new(Mutex::new(server_simple));
    let server_medium = Arc::new(Mutex::new(server_medium));
    let server_dynamic = Arc::new(Mutex::new(server_dynamic));

    // Spawn the drawing thread for the dynamic map
    std::thread::spawn(move || {
        let mut rng = rand::rng();
        let mut lat = 30.0;
        let mut lng = 106.5;
        let lng_step = 0.00252;
        let lat_step = lng_step;

        loop {
            // Add a new line segment
            {
                let next_lng = lng + lng_step;
                let next_lat = lat + rng.random_range(-lat_step..=lat_step);
                let mut map_renderer = map_renderer_arc_clone.lock().unwrap();
                map_renderer.update(|bitmap, _tile_cb| {
                    bitmap.add_line(lng, lat, next_lng, next_lat);
                });
                lng = next_lng;
                lat = next_lat;
            }

            // Sleep for 200ms
            std::thread::sleep(Duration::from_millis(200));
        }
    });

    // Set up keyboard input handling thread
    let server_simple_clone = server_simple.clone();
    let server_medium_clone = server_medium.clone();
    let server_dynamic_clone = server_dynamic.clone();
    std::thread::spawn(move || {
        println!("Press Ctrl+C to exit");

        // Enable raw mode to get immediate keystrokes
        enable_raw_mode().expect("Failed to enable raw mode");

        loop {
            // Check for keyboard events
            if let Ok(Event::Key(KeyEvent {
                code, modifiers, ..
            })) = event::read()
            {
                match code {
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        // Handle Ctrl+C manually since we're in raw mode
                        disable_raw_mode().expect("Failed to disable raw mode");

                        println!("Ctrl+C pressed. Stopping all servers...");
                        // Stop all three servers
                        if let Ok(mut server) = server_simple_clone.lock() {
                            let _ = server.stop();
                        }
                        if let Ok(mut server) = server_medium_clone.lock() {
                            let _ = server.stop();
                        }
                        if let Ok(mut server) = server_dynamic_clone.lock() {
                            let _ = server.stop();
                        }
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        }
    });

    // Block the main thread to keep all servers running
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
