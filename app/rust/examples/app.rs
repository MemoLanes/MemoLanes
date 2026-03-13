use memolanes_core::api::api::for_testing::get_main_map_state;
use memolanes_core::api::api::{analyze_mldx_import, import_journeys, init};
mod shared;
use memolanes_core::renderer::MapRenderer;
use shared::MapServer;
use std::env;
use std::sync::{Arc, Mutex};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    init(
        ".".to_string(),
        ".".to_string(),
        ".".to_string(),
        ".".to_string(),
    );

    // Check if an MLDX file path is provided as an argument
    if args.len() > 1 {
        let mldx_file_path = &args[1];
        println!("Importing MLDX file: {mldx_file_path}");
        match analyze_mldx_import(mldx_file_path.to_string()) {
            Ok(preview) => {
                let n = preview.journey.len();
                import_journeys(preview.journey)?;
                println!(
                    "Successfully imported {} journey(s) (skipped {} identical)",
                    n,
                    preview.skipped_count
                );
                if preview.conflict_count > 0 {
                    println!(
                        "{} of them overwrote existing conflicting journey(s).",
                        preview.conflict_count
                    );
                }
            }
            Err(e) => eprintln!("Failed to import MLDX file: {e:?}"),
        }
        return Ok(());
    }

    // HACK: we just make a full copy here, thus later updates won't be reflected in the server.
    let main_map_state = get_main_map_state();
    let journey_bitmap = main_map_state
        .lock()
        .unwrap()
        .map_renderer
        .peek_latest_bitmap()
        .clone();
    let server = MapServer::create_and_start(
        "localhost",
        None,
        Arc::new(Mutex::new(MapRenderer::new(journey_bitmap))),
    )
    .expect("Failed to start server");

    println!("view map at: {}", server.get_http_url());

    let _server = Arc::new(Mutex::new(server));

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
