extern crate simplelog;
use anyhow::Result;
use chrono::Utc;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::cache_db::CacheDb;
use crate::gps_processor::{self, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::main_db::{self, MainDb};
use crate::merged_journey_builder;

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

    fn record(
        &mut self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
        recevied_timestamp_ms: i64,
    ) {
        // TODO: better error handling
        let file = self.file.get_or_insert_with(|| {
            let timestamp_sec = Utc::now().timestamp_micros() / 1000000;
            let mut i = 0;
            let filename = loop {
                let filename =
                    Path::new(&self.dir).join(format!("gps-{}-{}.csv", timestamp_sec, i));
                if std::fs::metadata(&filename).is_err() {
                    break filename;
                }
                i += 1;
            };
            let mut file = File::create(filename).unwrap();
            let _ = file
                .write(
                    "timestamp_ms,recevied_timestamp_ms,latitude,longitude,accuarcy,altitude,speed,process_result\n"
                        .as_bytes(),
                )
                .unwrap();
            file
        });
        let _ = file
            .write(
                format!(
                    "{},{},{},{},{},{},{},{}\n",
                    raw_data.timestamp_ms.unwrap_or_default(),
                    recevied_timestamp_ms,
                    raw_data.latitude,
                    raw_data.longitude,
                    raw_data.accuracy.map(|x| x.to_string()).unwrap_or_default(),
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
    raw_data_recorder: Mutex<Option<RawDataRecorder>>, // `None` means disabled
    _cache_dir: String,
    // TODO: I feel the abstraction between `dbs`, `merged_journey_builder`, and
    // `main_map_renderer_need_to_reload` is a bit bad. We should refactor it,
    // but maybe do that when we know more.
    // NOTE: both db are deliberately hidden so all operations need to go
    // through `Storage` to make sure they are in sync.
    dbs: Mutex<(MainDb, CacheDb)>,
    main_map_renderer_need_to_reload: Mutex<bool>,
}

impl Storage {
    pub fn init(
        _temp_dir: String,
        _doc_dir: String,
        support_dir: String,
        cache_dir: String,
    ) -> Self {
        let mut main_db = MainDb::open(&support_dir);
        let cache_db = CacheDb::open(&cache_dir);
        let raw_data_recorder =
            if main_db.get_setting_with_default(crate::main_db::Setting::RawDataMode, false) {
                Some(RawDataRecorder::init(&support_dir))
            } else {
                None
            };
        Storage {
            support_dir,
            raw_data_recorder: Mutex::new(raw_data_recorder),
            _cache_dir: cache_dir,
            dbs: Mutex::new((main_db, cache_db)),
            main_map_renderer_need_to_reload: Mutex::new(true),
        }
    }

    pub fn with_db_txn<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&mut main_db::Txn) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let (ref mut main_db, ref cache_db) = *dbs;
        main_db.with_txn(|txn| {
            let output = f(txn)?;
            if txn.reset_cache {
                cache_db.clear_journey_cache()?;
                let mut main_map_renderer_need_to_reload =
                    self.main_map_renderer_need_to_reload.lock().unwrap();
                *main_map_renderer_need_to_reload = true;
            }
            Ok(output)
        })
    }

    pub fn toggle_raw_data_mode(&self, enable: bool) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if enable {
            if raw_data_recorder.is_none() {
                *raw_data_recorder = Some(RawDataRecorder::init(&self.support_dir));
                debug!("[storage] raw data mod enabled");
                let main_db = &mut self.dbs.lock().unwrap().0;
                main_db
                    .set_setting(crate::main_db::Setting::RawDataMode, true)
                    .unwrap();
            }
        } else if raw_data_recorder.is_some() {
            debug!("[storage] raw data mod disabled");
            // `drop` should do the right thing and release all resources.
            *raw_data_recorder = None;
            let main_db = &mut self.dbs.lock().unwrap().0;
            main_db
                .set_setting(crate::main_db::Setting::RawDataMode, false)
                .unwrap();
        }
    }

    pub fn get_raw_data_mode(&self) -> bool {
        let raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        raw_data_recorder.is_some()
    }

    pub fn record_gps_data(
        &self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
        recevied_timestamp_ms: i64,
    ) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.record(raw_data, process_result, recevied_timestamp_ms);
        }
        drop(raw_data_recorder);

        let main_db = &mut self.dbs.lock().unwrap().0;
        main_db.record(raw_data, process_result).unwrap();
    }

    pub fn list_all_raw_data(&self) -> Vec<RawDataFile> {
        // TODO: this is way too naive, implement a better one.
        let dir = Path::new(&self.support_dir).join("raw_data/");
        let mut result = Vec::new();
        if !dir.exists() {
            return result;
        }
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

    pub fn main_map_renderer_need_to_reload(&self) -> bool {
        let main_map_renderer_need_to_reload =
            self.main_map_renderer_need_to_reload.lock().unwrap();
        *main_map_renderer_need_to_reload
    }

    pub fn get_latest_bitmap_for_main_map_renderer(&self) -> Result<JourneyBitmap> {
        let mut dbs = self.dbs.lock().unwrap();
        let (ref mut main_db, ref cache_db) = *dbs;
        // passing `main_db` to `get_latest_including_ongoing` directly is fine
        // becuase it only reads `main_db`.
        let journey_bitmap =
            merged_journey_builder::get_latest_including_ongoing(main_db, cache_db)?;
        drop(dbs);

        let mut main_map_renderer_need_to_reload =
            self.main_map_renderer_need_to_reload.lock().unwrap();
        *main_map_renderer_need_to_reload = false;

        Ok(journey_bitmap)
    }

    // TODO: do we need this?
    pub fn _flush(&self) -> Result<()> {
        debug!("[storage] flushing");

        let dbs = self.dbs.lock().unwrap();
        dbs.0.flush()?;
        dbs.1.flush()?;
        drop(dbs);

        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.flush();
        }
        drop(raw_data_recorder);

        Ok(())
    }
}
