extern crate simplelog;
use anyhow::Result;
use chrono::Utc;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::gps_processor::{self, ProcessResult};
use crate::main_db::MainDb;

// TODO: error handling in this file is horrifying, we should think about what
// is the right thing to do here.

pub struct RawDataFile {
    pub name: String,
    pub path: String,
}

/* This is an optional feature that should be off by default: storing raw GPS
   data with detailed tempstamp. It is designed for advanced user or debugging.
   It stores data in a simple csv format and will be using a new file every time
   the app starts.

   TODO: we should zstd all old data to reduce disk usage.
*/
struct RawDataRecorder {
    dir: PathBuf,
    file: Option<File>,
}

impl RawDataRecorder {
    fn init(support_dir: &str) -> RawDataRecorder {
        // TODO: better error handling
        let dir = Path::new(support_dir).join("raw_data/");
        std::fs::create_dir_all(&dir).unwrap();
        RawDataRecorder { dir, file: None }
    }

    fn flush(&mut self) {
        if let Some(ref mut file) = self.file {
            file.flush().unwrap()
        }
    }

    fn record(&mut self, raw_data: &gps_processor::RawData, process_result: ProcessResult) {
        // TODO: better error handling
        let file = self.file.get_or_insert_with(|| {
            let timestamp_sec = Utc::now().timestamp_micros() / 1000000;
            let mut i = 0;
            let filename = loop {
                let filename =
                    Path::new(&self.dir).join(format!("gps-{}-{}.csv", timestamp_sec, i));
                if !std::fs::metadata(&filename).is_ok() {
                    break filename;
                }
                i += 1;
            };
            let mut file = File::create(filename).unwrap();
            file.write(
                "timestamp_ms,latitude,longitude,accuarcy,altitude,speed,process_result\n"
                    .as_bytes(),
            )
            .unwrap();
            file
        });
        file.write(
            format!(
                "{},{},{},{},{},{},{}\n",
                raw_data.timestamp_ms,
                raw_data.latitude,
                raw_data.longitude,
                raw_data.accuracy,
                &raw_data.altitude.map(|x| x.to_string()).unwrap_or_default(),
                &raw_data.speed.map(|x| x.to_string()).unwrap_or_default(),
                process_result.to_int()
            )
            .as_bytes(),
        )
        .unwrap();
    }
}

pub struct Storage {
    support_dir: String,
    main_db: Mutex<MainDb>,
    raw_data_recorder: Mutex<Option<RawDataRecorder>>, // `None` means disabled
}

impl Storage {
    pub fn init(
        _temp_dir: String,
        _doc_dir: String,
        support_dir: String,
        _cache_dir: String,
    ) -> Self {
        let main_db = MainDb::open(&support_dir);
        Storage {
            support_dir,
            main_db: Mutex::new(main_db),
            raw_data_recorder: Mutex::new(None),
        }
    }

    pub fn toggle_raw_data_mode(&self, enable: bool) {
        let mut raw_data_db = self.raw_data_recorder.lock().unwrap();
        if enable {
            if raw_data_db.is_none() {
                *raw_data_db = Some(RawDataRecorder::init(&self.support_dir));
                debug!("[storage] raw data mod enabled");
            }
        } else {
            if raw_data_db.is_some() {
                debug!("[storage] raw data mod disabled");
                // `drop` should do the right thing and release all resources.
                *raw_data_db = None;
            }
        }
    }

    pub fn record_gps_data(
        &self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
    ) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.record(raw_data, process_result);
        }
        drop(raw_data_recorder);

        let mut main_db = self.main_db.lock().unwrap();
        main_db.record(raw_data, process_result).unwrap();
    }

    pub fn list_all_raw_data(&self) -> Vec<RawDataFile> {
        // TODO: this is way too naive, implement a better one.
        let dir = Path::new(&self.support_dir).join("raw_data/");
        let mut result = Vec::new();
        for path in std::fs::read_dir(dir).unwrap() {
            let file = path.unwrap();
            let filename = file.file_name().to_str().unwrap().to_string();
            if filename.ends_with(".csv") {
                result.push(RawDataFile {
                    name: filename,
                    path: file.path().to_str().unwrap().to_owned(),
                })
            }
        }
        result
    }

    // TODO: do we need this?
    pub fn _flush(&self) -> Result<()> {
        debug!("[storage] flushing");

        let main_db = self.main_db.lock().unwrap();
        main_db.flush()?;
        drop(main_db);

        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.flush();
        }
        drop(raw_data_recorder);

        Ok(())
    }
}
