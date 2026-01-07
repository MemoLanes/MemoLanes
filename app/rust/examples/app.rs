#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

use memolanes_core::api::api::{
    get_map_renderer_proxy_for_main_map, get_registry, import_archive, init,
};
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

    let proxy = get_map_renderer_proxy_for_main_map();

    // Extract the MapRendererToken from the MapRendererProxy
    let memolanes_core::api::api::MapRendererProxy::Token(token) = &proxy;

    let registry = get_registry();
    let server = Arc::new(Mutex::new(
        MapServer::create_and_start_with_registry("localhost", None, registry)
            .expect("Failed to start server"),
    ));

    println!(
        "view map at: {}&debug=true",
        server.lock().unwrap().get_http_url(token)
    );

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
