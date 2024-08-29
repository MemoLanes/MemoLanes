use std::{
    fs::{self, File}, io, path::Path
};

use anyhow::Result;
use file_rotate::{
    compression::Compression,
    suffix::{AppendTimestamp, FileLimit},
    {ContentLimit, FileRotate},
};
use simplelog::{Config, LevelFilter, WriteLogger};

pub fn init(cache_dir: &str) -> Result<()> {
    let path = Path::new(cache_dir).join("logs/main.log");
    let log = FileRotate::new(
        path,
        AppendTimestamp::default(FileLimit::MaxFiles(3)),
        ContentLimit::Lines(1000),
        Compression::None,
        None,
    );
    WriteLogger::init(LevelFilter::Info, Config::default(), log)?;
    Ok(())
}

pub fn export(cache_dir: &str, target_file_path: &str) -> Result<()> {
    let mut zip = zip::ZipWriter::new(File::open(target_file_path)?);
    let default_options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let log_folder = Path::new(cache_dir).join("logs/");
    for entry in fs::read_dir(&log_folder)? {
        if let Some(entry) = entry.ok() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.strip_prefix(&log_folder)?.to_str() {
                    zip.start_file(name, default_options)?;
                    let mut log_file = File::open(path)?;
                    io::copy(&mut log_file, &mut zip)?;
                }
            }
        };
    }

    zip.finish()?;
    Ok(())
}
