use std::{
    fs::{self, File},
    io,
    path::Path,
};

use crate::frb_generated::StreamSink;
use anyhow::{Context, Result};
use auto_context::auto_context;
use file_rotate::{
    compression::Compression,
    suffix::{AppendTimestamp, FileLimit},
    {ContentLimit, FileRotate},
};
use log::Log;
use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};
use std::sync::{LazyLock, Mutex};

pub static FLUTTER_LOGGER: LazyLock<Mutex<Option<StreamSink<String>>>> =
    LazyLock::new(|| Mutex::new(None));

pub struct MainLogger {
    write_logger: Box<WriteLogger<FileRotate<AppendTimestamp>>>,
}

impl MainLogger {
    fn new(write_logger: Box<WriteLogger<FileRotate<AppendTimestamp>>>) -> Self {
        Self { write_logger }
    }
}

impl Log for MainLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if let Some(ref mut sink) = *FLUTTER_LOGGER.lock().unwrap() {
            let message = format!(
                "{}:{} -- {}",
                record.level(),
                record.target(),
                record.args()
            );
            let _ = sink.add(message);
        }
        self.write_logger.log(record);
    }

    fn flush(&self) {}
}

#[auto_context]
pub fn init(cache_dir: &str) -> Result<()> {
    let path = Path::new(cache_dir).join("logs/main.log");
    let log = FileRotate::new(
        path,
        AppendTimestamp::default(FileLimit::MaxFiles(3)),
        ContentLimit::Lines(1000),
        Compression::None,
        None,
    );
    let config = ConfigBuilder::new().set_time_format_rfc3339().build();
    let write_logger = WriteLogger::new(LevelFilter::Info, config, log);
    let main_logger = MainLogger::new(write_logger);
    log::set_boxed_logger(Box::new(main_logger))?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}

#[auto_context]
pub fn export(cache_dir: &str, target_file_path: &str) -> Result<()> {
    let mut zip = zip::ZipWriter::new(File::create(target_file_path)?);
    let default_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let log_folder = Path::new(cache_dir).join("logs/");
    for entry in (fs::read_dir(&log_folder)?).flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.strip_prefix(cache_dir)?.to_str() {
                zip.start_file(name, default_options)?;
                let mut log_file = File::open(path)?;
                io::copy(&mut log_file, &mut zip)?;
            }
        }
    }

    zip.finish()?;
    Ok(())
}
