use memolanes_core::api::api::for_testing::get_main_map_state;
use memolanes_core::api::api::{import_mldx, init, open_mldx_file};
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
        let mldx_file = open_mldx_file(mldx_file_path.to_string())?;
        match import_mldx(&mldx_file, None) {
            Ok(()) => {
                println!("Successfully imported archive.");
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
