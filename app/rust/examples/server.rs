use clap::Parser;
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::renderer::MapRenderer;
mod shared;
use shared::MapServer;

use rand::RngExt;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Parser)]
#[command(about = "Serve demo maps, or a single GPX file, through the map API")]
struct Cli {
    /// Serve only this GPX file instead of the three demo maps.
    #[arg(long, value_name = "PATH")]
    gpx: Option<String>,
}

const START_LNG: f64 = 151.14;
const START_LAT: f64 = -33.79;
const END_LNG: f64 = 141.27;
const END_LAT: f64 = -25.94;
const MID_LNG: f64 = (START_LNG + END_LNG) / 2.;
const MID_LAT: f64 = (START_LAT + END_LAT) / 2.;

fn draw_line1(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, START_LAT, END_LNG, END_LAT)
}
fn draw_line2(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, END_LAT, END_LNG, START_LAT);
}
fn draw_line3(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(MID_LNG, START_LAT, MID_LNG, END_LAT)
}
fn draw_line4(journey_bitmap: &mut JourneyBitmap) {
    journey_bitmap
        .edit()
        .add_line(START_LNG, MID_LAT, END_LNG, MID_LAT)
}

fn draw_line5(journey_bitmap: &mut JourneyBitmap) {
    // Draw a line from Tokyo to Hawaii across the antimeridian
    journey_bitmap
        .edit()
        .add_line(139.7690, 35.6804, -157.8583, 21.3069)
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    // Initialize the logger with info level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_module_path(false)
        .init();

    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_for_handler = shutdown_requested.clone();
    ctrlc::set_handler(move || shutdown_for_handler.store(true, Ordering::SeqCst))?;

    let mut servers = if let Some(path) = cli.gpx {
        vec![start_gpx_server(&path)?]
    } else {
        println!("Run `yarn dev` in journey_kernel to serve the map page.");
        println!("Set DEV_SERVER to override the default http://localhost:8080 page URL.");
        start_demo_servers(shutdown_requested.clone())?
    };

    println!("Press Ctrl+C to exit");
    std::io::stdout().flush()?;
    while !shutdown_requested.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    for server in &mut servers {
        server.stop()?;
    }
    Ok(())
}

fn start_gpx_server(path: &str) -> Result<MapServer, Box<dyn std::error::Error>> {
    let (raw, _) = import_data::gpx::load_gpx(path)?;
    let vector =
        import_data::conversion::journey_vector_from_raw_data_with_gps_preprocessor(&raw, None)
            .ok_or_else(|| anyhow::anyhow!("GPX contains no valid track points"))?;
    let mut bitmap = JourneyBitmap::new();
    bitmap.edit().merge_vector(&vector);
    let server = MapServer::create_and_start(Arc::new(Mutex::new(MapRenderer::new(bitmap))))?;
    println!("[GPX Map Server]: {}", server.get_http_url());
    println!("MAP_ENDPOINT={}", server.get_api_endpoint());
    Ok(server)
}

fn start_demo_servers(
    shutdown_requested: Arc<AtomicBool>,
) -> Result<Vec<MapServer>, Box<dyn std::error::Error>> {
    // ========== Server 1: Simple Map (static with crossed lines) ==========
    let mut journey_bitmap = JourneyBitmap::new();
    draw_line1(&mut journey_bitmap);
    draw_line2(&mut journey_bitmap);
    draw_line3(&mut journey_bitmap);
    draw_line4(&mut journey_bitmap);
    draw_line5(&mut journey_bitmap);

    let map_renderer_static = Arc::new(Mutex::new(MapRenderer::new(journey_bitmap)));
    let server_simple = MapServer::create_and_start(map_renderer_static)?;

    println!("================================================");
    println!("[Simple Map Server]:   {}", server_simple.get_http_url());

    // ========== Server 2: Medium Map (loaded from fow_3.zip) ==========
    let (journey_bitmap_fow, _) = import_data::fow::load_fow_sync_data("./tests/data/fow_3.zip")?;
    let map_renderer_fow = Arc::new(Mutex::new(MapRenderer::new(journey_bitmap_fow)));
    let server_medium = MapServer::create_and_start(map_renderer_fow)?;

    println!("[Medium Map Server]:   {}", server_medium.get_http_url());

    // ========== Server 3: Dynamic Map (randomly drawn lines) ==========
    let journey_bitmap2 = JourneyBitmap::new();
    let map_renderer_dynamic = Arc::new(Mutex::new(MapRenderer::new(journey_bitmap2)));
    let map_renderer_arc_clone = map_renderer_dynamic.clone();
    let server_dynamic = MapServer::create_and_start(map_renderer_dynamic)?;

    println!("[Dynamic Map Server]:  {}", server_dynamic.get_http_url());

    // Spawn the drawing thread for the dynamic map
    std::thread::spawn(move || {
        let mut rng = rand::rng();
        let mut lat = 30.0;
        let mut lng = 106.5;
        let lng_step = 0.00252;
        let lat_step = lng_step;

        while !shutdown_requested.load(Ordering::SeqCst) {
            // Add a new line segment
            {
                let next_lng = lng + lng_step;
                let next_lat = lat + rng.random_range(-lat_step..=lat_step);
                let mut map_renderer = map_renderer_arc_clone.lock().unwrap();
                map_renderer.update(|editor| {
                    editor.add_line(lng, lat, next_lng, next_lat);
                });
                lng = next_lng;
                lat = next_lat;
            }

            // Sleep for 200ms
            std::thread::sleep(Duration::from_millis(200));
        }
    });

    Ok(vec![server_simple, server_medium, server_dynamic])
}
