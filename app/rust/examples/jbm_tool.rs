use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use clap::Parser;
use memolanes_core::api::import::ImportPreprocessor;
use memolanes_core::archive::MldxReader;
use memolanes_core::gps_processor::SegmentGapRule;
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::JourneyBitmap;
use memolanes_core::journey_data;
use memolanes_core::renderer::MapRenderer;

mod shared;
use shared::MapServer;

#[derive(Parser, Debug)]
#[command(
    name = "jbm_tool",
    about = "Import, export, and preview journey bitmap data",
    long_about = "Reads journey data from one or more sources (GPX/KML tracks, MLDX archives, \
                  JBM files, or an existing MemoLanes app directory), merges them into a single \
                  journey bitmap, and optionally writes a .jbm file and/or serves it via a map server."
)]
struct Cli {
    /// Output .jbm file path.
    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<String>,

    /// Input GPX, KML, MLDX, or JBM files (can be specified multiple times).
    #[arg(short, long = "file", value_name = "FILE")]
    files: Vec<String>,

    /// Read from an existing MemoLanes app data directory.
    #[arg(short, long = "data-dir", value_name = "DIR")]
    data_dir: Option<String>,

    /// Start a map server to view the result in a browser.
    #[arg(short, long)]
    serve: bool,
}

fn process_gpx_or_kml(file_path: &str) -> Result<JourneyBitmap> {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let (raw_data, preprocessor) = match ext.as_deref() {
        Some("gpx") => import_data::load_gpx(file_path)?,
        Some("kml") => import_data::load_kml(file_path)?,
        _ => bail!("Unsupported file extension: {ext:?}"),
    };

    let gap_rule = match preprocessor {
        ImportPreprocessor::None => None,
        ImportPreprocessor::Generic => Some(SegmentGapRule::Default),
        ImportPreprocessor::Spare => Some(SegmentGapRule::Spare),
        ImportPreprocessor::FlightTrack => {
            let jv = memolanes_core::flight_track_processor::process(&raw_data);
            let mut bm = JourneyBitmap::new();
            if let Some(v) = jv {
                bm.merge_vector(&v);
            }
            return Ok(bm);
        }
    };

    let jv = import_data::journey_vector_from_raw_data_with_gps_preprocessor(&raw_data, gap_rule);
    let mut bm = JourneyBitmap::new();
    if let Some(v) = jv {
        bm.merge_vector(&v);
    }
    Ok(bm)
}

fn process_mldx(file_path: &str) -> Result<JourneyBitmap> {
    let file =
        File::open(file_path).with_context(|| format!("Failed to open MLDX file: {file_path}"))?;
    let mut reader = MldxReader::open(file)?;

    let journey_ids: Vec<String> = reader
        .iter_journey_headers()
        .iter()
        .map(|h| h.id.clone())
        .collect();

    let mut bitmap = JourneyBitmap::new();
    for id in &journey_ids {
        if let Some((_header, data)) = reader.load_single_journey(id)? {
            data.merge_into(&mut bitmap);
        }
    }
    Ok(bitmap)
}

fn process_jbm(file_path: &str) -> Result<JourneyBitmap> {
    let file =
        File::open(file_path).with_context(|| format!("Failed to open JBM file: {file_path}"))?;
    let bitmap = journey_data::deserialize_journey_bitmap(file, true)?;
    Ok(bitmap)
}

fn process_data_dir(dir: &str) -> Result<JourneyBitmap> {
    use memolanes_core::api::api::{for_testing::get_main_map_state, init, init_main_map};

    init(
        dir.to_string(),
        dir.to_string(),
        dir.to_string(),
        dir.to_string(),
    );
    init_main_map()?;

    let main_map_state = get_main_map_state();
    let bitmap = main_map_state
        .lock()
        .unwrap()
        .map_renderer
        .peek_latest_bitmap()
        .clone();
    Ok(bitmap)
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.files.is_empty() && cli.data_dir.is_none() {
        bail!("No input specified. Provide --file and/or --data-dir.");
    }
    if cli.output.is_none() && !cli.serve {
        bail!("No action specified. Provide --output and/or --serve.");
    }

    let mut bitmap = JourneyBitmap::new();

    if let Some(dir) = &cli.data_dir {
        println!("Loading app data from: {dir}");
        let dir_bitmap = process_data_dir(dir)?;
        bitmap.merge(dir_bitmap);
        println!("  done.");
    }

    for file_path in &cli.files {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        println!("Processing: {file_path}");
        let file_bitmap = match ext.as_deref() {
            Some("gpx") | Some("kml") => process_gpx_or_kml(file_path)?,
            Some("mldx") => process_mldx(file_path)?,
            Some("jbm") => process_jbm(file_path)?,
            other => bail!("Unsupported file type: {other:?} (file: {file_path})"),
        };
        bitmap.merge(file_bitmap);
        println!("  done.");
    }

    if let Some(output) = cli.output {
        let output_path = if output.ends_with(".jbm") {
            output
        } else {
            format!("{output}.jbm")
        };

        let file = File::create(&output_path)
            .with_context(|| format!("Failed to create output file: {output_path}"))?;
        journey_data::serialize_journey_bitmap(&mut bitmap, file)?;
        println!("Exported journey bitmap to: {output_path}");
    }

    if cli.serve {
        let server = MapServer::create_and_start(Arc::new(Mutex::new(MapRenderer::new(bitmap))))
            .expect("Failed to start server");

        println!("View map at: {}", server.get_http_url());
        if let Err(e) = qr2term::print_qr(server.get_http_url()) {
            eprintln!("Failed to print QR code: {e}");
        }
        println!("Press Ctrl+C to exit");

        let _server = Arc::new(Mutex::new(server));

        ctrlc::set_handler(move || {
            println!("\nShutting down...");
            std::process::exit(0);
        })?;

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    Ok(())
}
