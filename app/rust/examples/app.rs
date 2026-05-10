use clap::Parser;
use memolanes_core::api::api::for_testing::get_main_map_state;
use memolanes_core::api::api::{init, init_main_map};
use memolanes_core::journey_data::serialize_journey_bitmap;
mod shared;
use memolanes_core::api::import::OpaqueMldxReader;
use memolanes_core::renderer::MapRenderer;
use shared::MapServer;
use std::fs::File;
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(
    name = "app",
    about = "Run MemoLanes core in current directory",
    long_about = "Run the MemoLanes core in the current directory. Start the local map server, import an MLDX archive, or export the current bitmap as a JBM file."
)]
struct Cli {
    /// Export the main map journey bitmap to a .jbm file.
    #[arg(
        long = "export-jbm",
        value_name = "OUTPUT_PATH",
        conflicts_with_all = ["mldx_file", "import_mldx"]
    )]
    export_jbm: Option<String>,

    /// Import an MLDX archive file.
    #[arg(
        long = "import-mldx",
        value_name = "MLDX_FILE",
        conflicts_with = "mldx_file"
    )]
    import_mldx: Option<String>,

    /// Import an MLDX archive file.
    #[arg(value_name = "MLDX_FILE")]
    mldx_file: Option<String>,
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    init(
        ".".to_string(),
        ".".to_string(),
        ".".to_string(),
        ".".to_string(),
    );

    init_main_map()?;

    if let Some(output_path) = cli.export_jbm {
        let output_path = if output_path.ends_with(".jbm") {
            output_path
        } else {
            format!("{output_path}.jbm")
        };

        let mut journey_bitmap = get_main_map_state()
            .lock()
            .unwrap()
            .map_renderer
            .peek_latest_bitmap()
            .clone();
        let file = File::create(&output_path)?;
        serialize_journey_bitmap(&mut journey_bitmap, file)?;
        println!("Exported journey bitmap to: {output_path}");
        return Ok(());
    }

    // Check if an MLDX file path is provided as an option or positional argument.
    if let Some(mldx_file_path) = cli.import_mldx.or(cli.mldx_file) {
        println!("Importing MLDX file: {mldx_file_path}");
        let mldx_file = OpaqueMldxReader::open(mldx_file_path.to_string())?;
        match mldx_file.import_journeys(None) {
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
    let server =
        MapServer::create_and_start(Arc::new(Mutex::new(MapRenderer::new(journey_bitmap))))
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
