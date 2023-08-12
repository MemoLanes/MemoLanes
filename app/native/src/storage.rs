extern crate simplelog;
use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use std::path::Path;

pub struct Storage {
    temp_dir: String,
    doc_dir: String,
    support_dir: String,
    cache_dir: String,
}

impl Storage {
    pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) -> Self {
        Storage {
            temp_dir,
            doc_dir,
            support_dir,
            cache_dir,
        }
    }

    pub fn init_logging(&self) {
        let path = Path::new(&self.temp_dir).join("main.log");
        WriteLogger::init(
            LevelFilter::Info,
            Config::default(),
            File::create(path).unwrap(),
        )
        .expect("Failed to initialize logging");
    }
}
