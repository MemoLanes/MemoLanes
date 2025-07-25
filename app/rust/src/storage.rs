extern crate simplelog;
use anyhow::{Ok, Result};
use chrono::Local;
use std::collections::HashMap;
use std::fs::{remove_file, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::cache_db::{CacheDb, LayerKind};
use crate::gps_processor::{self, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;
use crate::main_db::{self, Action, MainDb};
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
    file_and_name: Option<(File, String)>,
}

impl RawDataRecorder {
    fn init(support_dir: &str) -> RawDataRecorder {
        // TODO: better error handling
        let dir = Path::new(support_dir).join("raw_data/");
        std::fs::create_dir_all(&dir).unwrap();
        RawDataRecorder {
            dir,
            file_and_name: None,
        }
    }

    fn flush(&mut self) {
        if let Some(ref mut file_and_name) = self.file_and_name {
            file_and_name.0.flush().unwrap()
        }
    }

    fn record(&mut self, raw_data: &gps_processor::RawData, recevied_timestamp_ms: i64) {
        // TODO: better error handling
        let (file, _) = self.file_and_name.get_or_insert_with(|| {
            let current_date = Local::now().date_naive();
            let mut i = 0;
            let (path,filename) = loop {
                let filename = format!("gps-{current_date}-{i}.csv");
                let path =
                    Path::new(&self.dir).join(&filename);
                if std::fs::metadata(&path).is_err() {
                    break (path,filename);
                }
                i += 1;
            };
            let mut file = File::create(path).unwrap();
            let _ = file
                .write(
                    "timestamp_ms,recevied_timestamp_ms,latitude,longitude,accuarcy,altitude,speed\n"
                        .as_bytes(),
                )
                .unwrap();
            (file,filename)
        });
        file.write_all(
            format!(
                "{},{},{},{},{},{},{}\n",
                raw_data.timestamp_ms.unwrap_or_default(),
                recevied_timestamp_ms,
                raw_data.point.latitude,
                raw_data.point.longitude,
                raw_data.accuracy.map(|x| x.to_string()).unwrap_or_default(),
                &raw_data.altitude.map(|x| x.to_string()).unwrap_or_default(),
                &raw_data.speed.map(|x| x.to_string()).unwrap_or_default()
            )
            .as_bytes(),
        )
        .unwrap();
    }
}

type FinalizedJourneyChangedCallback = Box<dyn Fn(&Storage) + Send + Sync + 'static>;

pub struct Storage {
    support_dir: String,
    raw_data_recorder: Mutex<Option<RawDataRecorder>>, // `None` means disabled
    pub cache_dir: String,
    // TODO: I feel the abstraction between `dbs`, `merged_journey_builder`, and
    // `main_map_renderer_need_to_reload` is a bit bad. We should refactor it,
    // but maybe do that when we know more.
    // NOTE: both db are deliberately hidden so all operations need to go
    // through `Storage` to make sure they are in sync.
    dbs: Mutex<(MainDb, CacheDb)>,
    finalized_journey_changed_callback: FinalizedJourneyChangedCallback,
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
            cache_dir,
            dbs: Mutex::new((main_db, cache_db)),
            finalized_journey_changed_callback: Box::new(|_| {}),
        }
    }

    pub fn with_db_txn<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&mut main_db::Txn) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let (ref mut main_db, ref cache_db) = *dbs;

        let mut finalized_journey_changed = false;

        let output = main_db.with_txn(|txn| {
            let output = f(txn)?;

            match &txn.action {
                None => (),
                Some(action) => {
                    match action {
                        Action::CompleteRebuilt => {
                            cache_db.clear_all_cache()?;
                        }
                        Action::Merge { journey_ids } => {
                            // TODO: This implementation is pretty naive, but we might not need it when we have cache v3
                            cache_db.delete_full_journey_cache(&LayerKind::All)?;

                            let mut kind_id_map: HashMap<JourneyKind, Vec<String>> = HashMap::new();

                            for journey_id in journey_ids {
                                if let Some(header) = txn.get_journey_header(journey_id)? {
                                    kind_id_map
                                        .entry(header.journey_kind)
                                        .or_default()
                                        .push(journey_id.clone());
                                }
                            }

                            for (kind, journeyid_vec) in kind_id_map {
                                let layer_kind = LayerKind::JounreyKind(kind);
                                cache_db.update_full_journey_cache_if_exists(&layer_kind, |current_cache| {
                                    for journey_id in journeyid_vec {
                                        let journey_data = txn.get_journey_data(&journey_id)?;
                                        match journey_data {
                                            JourneyData::Bitmap(bitmap) =>
                                                current_cache.merge(bitmap),
                                            JourneyData::Vector(vector) =>
                                                merged_journey_builder::add_journey_vector_to_journey_bitmap(
                                                    current_cache, &vector
                                            ),
                                        }
                                    }
                                    Ok(())
                                })?;
                            }
                        }
                    };
                    finalized_journey_changed = true;
                }
            }

            Ok(output)
        })?;

        // Make using we are not holding the lock when calling the callback
        // TODO: This is still error-prone, and easy to cause deadlock. Consider
        // using a separate thread to call the callback.
        drop(dbs);
        if finalized_journey_changed {
            (self.finalized_journey_changed_callback)(self);
        }

        Ok(output)
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

    pub fn delete_raw_data_file(&self, filename: String) -> Result<()> {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            if let Some((_, current_writing_filename)) = &x.file_and_name {
                if current_writing_filename == &filename {
                    x.file_and_name = None;
                }
            }
        }
        remove_file(
            Path::new(&self.support_dir)
                .join("raw_data/")
                .join(&filename),
        )?;
        Ok(())
    }

    pub fn record_gps_data(
        &self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
        recevied_timestamp_ms: i64,
    ) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.record(raw_data, recevied_timestamp_ms);
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
        result.sort_by(|a, b| a.name.cmp(&b.name).reverse());
        result
    }

    pub fn set_finalized_journey_changed_callback(
        &mut self,
        callback: FinalizedJourneyChangedCallback,
    ) {
        self.finalized_journey_changed_callback = callback;
    }

    pub fn get_latest_bitmap_for_main_map_renderer(
        &self,
        layer_kind: &LayerKind,
    ) -> Result<JourneyBitmap> {
        let mut dbs = self.dbs.lock().unwrap();
        let (ref mut main_db, ref cache_db) = *dbs;
        // passing `main_db` to `get_latest_including_ongoing` directly is fine
        // becuase it only reads `main_db`.
        let journey_bitmap =
            merged_journey_builder::get_latest_including_ongoing(main_db, cache_db, layer_kind)?;
        drop(dbs);

        Ok(journey_bitmap)
    }

    pub fn clear_all_cache(&self) -> Result<()> {
        let cache_db = &self.dbs.lock().unwrap().1;
        cache_db.clear_all_cache()?;
        Ok(())
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
