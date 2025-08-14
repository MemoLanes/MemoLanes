use std::{
    fs::{self, File},
    io,
    path::Path,
    sync::{mpsc, LazyLock, Mutex},
    thread,
};

use crate::frb_generated::StreamSink;
use anyhow::Result;
use file_rotate::{
    compression::Compression,
    suffix::{AppendTimestamp, FileLimit},
    {ContentLimit, FileRotate},
};
use log::Log;
use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};

pub static FLUTTER_LOGGER: LazyLock<Mutex<Option<StreamSink<String>>>> =
    LazyLock::new(|| Mutex::new(None));

pub static LOG_SENDER: LazyLock<Mutex<Option<mpsc::Sender<String>>>> =
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
        self.write_logger.log(record);

        let message = format!(
            "{}:{} -- {}",
            record.level(),
            record.target(),
            record.args()
        );

        if let Some(tx) = LOG_SENDER.lock().unwrap().as_ref() {
            let _ = tx.send(message);
        } else {
            eprintln!("{}", message);
        }
    }

    fn flush(&self) {}
}

pub fn init(cache_dir: &str) -> Result<()> {
    let path = Path::new(cache_dir).join("logs/main.log");
    let log = FileRotate::new(
        path,
        AppendTimestamp::default(FileLimit::MaxFiles(3)),
        ContentLimit::Lines(1000),
        Compression::None,
        #[cfg(unix)]
        None,
    );
    let config = ConfigBuilder::new().set_time_format_rfc3339().build();
    let write_logger = WriteLogger::new(LevelFilter::Info, config, log);
    let main_logger = MainLogger::new(write_logger);
    log::set_boxed_logger(Box::new(main_logger))?;
    log::set_max_level(LevelFilter::Info);

    init_dispatcher();

    Ok(())
}

fn init_dispatcher() {
    let mut guard = LOG_SENDER.lock().unwrap();
    if guard.is_some() {
        return;
    }

    let (tx, rx) = mpsc::channel::<String>();
    *guard = Some(tx);

    let flutter_logger_ref = &*FLUTTER_LOGGER;

    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            eprintln!("[DISPATCH] {}", msg);

            let maybe_sink = {
                let guard = flutter_logger_ref.lock().unwrap();
                guard.clone()
            };

            if let Some(mut sink) = maybe_sink {
                let _ = sink.add(msg);
            }
        }
    });
}

pub fn set_flutter_sink(sink: StreamSink<String>) {
    let mut guard = FLUTTER_LOGGER.lock().unwrap();
    *guard = Some(sink);
}

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
