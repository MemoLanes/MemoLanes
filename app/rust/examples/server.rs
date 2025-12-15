use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::renderer::internal_server::register_map_renderer;
use memolanes_core::renderer::internal_server::Registry;
use memolanes_core::renderer::MapRenderer;
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
    // Initialize the logger with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let registry = Arc::new(Mutex::new(Registry::new()));

    let server = Arc::new(Mutex::new(
        MapServer::create_and_start_with_registry("localhost", None, registry.clone())
            .expect("Failed to start server"),
    ));

    std::thread::sleep(Duration::from_millis(200));

    println!("############ BREAK CHANGE NOTICE ###############");
    println!("Starting from Dec 2025, user need to hosting the static resouces by themselves and the rust side will only handle the dynamic requests.");
    println!("Please make sure you have `yarn dev` running in journey_kernal folder.");
    println!(
        "- By default, we assume the static resources are available at `http://localhost:8080`"
    );
    println!(
        "- You may also pass the static resources url via the DEV_SERVER environment variable"
    );
    println!("    eg: `DEV_SERVER=http://localhost:8080 cargo run --release --example server -- --nocapture`");
    println!("################################################");

    // demo for a static map
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);

    let map_renderer_static = MapRenderer::new(journey_bitmap);
    let token = register_map_renderer(registry.clone(), Arc::new(Mutex::new(map_renderer_static)));

    println!("================================================");
    println!(
        "[Simple Map Server]:   {}",
        server.lock().unwrap().get_http_url(&token)
    );
    println!(
        "[Simple Map Local]:    {}",
        server.lock().unwrap().get_file_url(&token)
    );

    let (joruney_bitmap_fow, _) =
        import_data::load_fow_sync_data("./tests/data/fow_3.zip").unwrap();
    let map_renderer_fow = MapRenderer::new(joruney_bitmap_fow);
    let token_fow = server
        .lock()
        .unwrap()
        .register_map_renderer(Arc::new(Mutex::new(map_renderer_fow)));
    println!(
        "[Medium Map Server]:   {}",
        server.lock().unwrap().get_http_url(&token_fow)
    );

    // demo for a dynamic map
    let journey_bitmap2 = JourneyBitmap::new();
    let map_renderer = MapRenderer::new(journey_bitmap2);
    let map_renderer_arc = Arc::new(Mutex::new(map_renderer));
    let map_renderer_arc_clone = map_renderer_arc.clone();
    let token = server
        .lock()
        .unwrap()
        .register_map_renderer(map_renderer_arc);

    println!(
        "[Dynamic Map Server]:  {}",
        server.lock().unwrap().get_http_url(&token)
    );

    // Spawn the drawing thread
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
    let server_clone = server.clone();
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

                        println!("Ctrl+C pressed. Stopping server...");
                        if let Ok(mut server) = server_clone.lock() {
                            let _ = server.stop();
                        }
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        }
    });

    // Block the main thread to keep server running
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
