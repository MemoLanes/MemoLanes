use memolanes_core::api::api::{for_testing::get_main_map_renderer, import_archive, init};
mod shared;
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
        match import_archive(mldx_file_path.to_string()) {
            Ok(_) => println!("Successfully imported MLDX file"),
            Err(e) => eprintln!("Failed to import MLDX file: {e:?}"),
        }
        return Ok(());
    }

    // Get the main map renderer and create a registry with it
    let main_map_renderer = get_main_map_renderer();
    let registry = Arc::new(Mutex::new(Some(main_map_renderer.clone())));

    let mut server = MapServer::create_and_start_with_registry("localhost", None, registry)
        .expect("Failed to start server");

    // Register the main map renderer to get a token for URL generation
    let token = server.register_map_renderer(main_map_renderer);

    println!("view map at: {}", server.get_http_url(&token));

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
