use memolanes_core::import_data;
use memolanes_core::renderer::MapRenderer;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[allow(dead_code)]
mod shared;
use shared::MapServer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (journey_bitmap, _) = import_data::fow::load_fow_sync_data("./tests/data/fow_3.zip")?;
    let renderer = Arc::new(Mutex::new(MapRenderer::new(journey_bitmap)));
    let mut server = MapServer::create_and_start(renderer)?;

    println!("BENCHMARK_URL={}", server.get_http_url());
    std::io::stdout().flush()?;

    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let shutdown_requested_for_handler = shutdown_requested.clone();
    ctrlc::set_handler(move || {
        shutdown_requested_for_handler.store(true, Ordering::SeqCst);
    })?;

    while !shutdown_requested.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }

    server.stop()?;
    Ok(())
}
