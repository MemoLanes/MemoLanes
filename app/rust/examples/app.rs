use ctrlc;
use memolanes_core::api::api::{get_map_renderer_proxy_for_main_map, import_archive, init};
use std::env;

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
        println!("Importing MLDX file: {}", mldx_file_path);
        match import_archive(mldx_file_path.to_string()) {
            Ok(_) => println!("Successfully imported MLDX file"),
            Err(e) => eprintln!("Failed to import MLDX file: {:?}", e),
        }
        return Ok(());
    }

    let proxy = get_map_renderer_proxy_for_main_map();
    println!("view map at: {}&debug=true", proxy.get_url());

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
