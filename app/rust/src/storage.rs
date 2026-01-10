extern crate simplelog;
use crate::cache_db::{CacheDb, LayerKind};
use crate::gps_processor::{self, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;
use crate::main_db::{self, Action, MainDb};
use crate::merged_journey_builder;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{remove_file, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// TODO: error handling in this file is horrifying, we should think about what
// is the right thing to do here.

pub struct RawDataFile {
    pub name: String,
    pub path: String,
}

struct CurrentRawDataFile {
    writer: csv::Writer<File>,
    filename: String,
    date: chrono::NaiveDate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawCsvRow {
    pub timestamp_ms: Option<i64>,
    pub received_timestamp_ms: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: Option<f32>,
    pub altitude: Option<f32>,
    pub speed: Option<f32>,
}

impl From<(&gps_processor::RawData, i64)> for RawCsvRow {
    fn from((raw_data, received_timestamp_ms): (&gps_processor::RawData, i64)) -> Self {
        Self {
            timestamp_ms: raw_data.timestamp_ms,
            received_timestamp_ms,
            latitude: raw_data.point.latitude,
            longitude: raw_data.point.longitude,
            accuracy: raw_data.accuracy,
            altitude: raw_data.altitude,
            speed: raw_data.speed,
        }
    }
}

/* This is an optional feature that should be off by default: storing raw GPS
   data with detailed timestamp. It is designed for advanced user or debugging.
   It stores data in a simple csv format and will be using a new file every time
   the app starts.

   TODO: we should zstd all old data to reduce disk usage.
*/
struct RawDataRecorder {
    dir: PathBuf,
    current_raw_data_file: Option<CurrentRawDataFile>,
}

impl RawDataRecorder {
    fn init(support_dir: &str) -> RawDataRecorder {
        // TODO: better error handling
        let dir = Path::new(support_dir).join("raw_data/");
        std::fs::create_dir_all(&dir).unwrap();
        RawDataRecorder {
            dir,
            current_raw_data_file: None,
        }
    }

    fn flush(&mut self) {
        if let Some(ref mut current_raw_data_file) = self.current_raw_data_file {
            current_raw_data_file.writer.flush().unwrap();
        }
    }

    // TODO: better error handling
    fn record(&mut self, raw_data: &gps_processor::RawData, received_timestamp_ms: i64) {
        let current_date = Local::now().date_naive();
        if let Some(current_raw_data_file) = &self.current_raw_data_file {
            if current_raw_data_file.date != current_date {
                // date changed, start a new file
                self.current_raw_data_file = None;
            }
        }

        let current_raw_data_file = self.current_raw_data_file.get_or_insert_with(|| {
            let mut i = 0;
            let (path, filename) = loop {
                let filename = format!("gps-{current_date}-{i}.csv");
                let path = Path::new(&self.dir).join(&filename);
                if std::fs::metadata(&path).is_err() {
                    break (path, filename);
                }
                i += 1;
            };
            let file = File::create(path).unwrap();
            let writer = csv::WriterBuilder::new()
                .has_headers(true)
                .from_writer(file);

            CurrentRawDataFile {
                writer,
                filename,
                date: current_date,
            }
        });
        let row: RawCsvRow = (raw_data, received_timestamp_ms).into();
        current_raw_data_file.writer.serialize(row).unwrap();
        current_raw_data_file.writer.flush().unwrap();
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

    #[auto_context]
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
                                let layer_kind = LayerKind::JourneyKind(kind);
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
                info!("[storage] raw data mod enabled");
                let main_db = &mut self.dbs.lock().unwrap().0;
                main_db
                    .set_setting(crate::main_db::Setting::RawDataMode, true)
                    .unwrap();
            }
        } else if raw_data_recorder.is_some() {
            info!("[storage] raw data mod disabled");
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

    #[auto_context]
    pub fn delete_raw_data_file(&self, filename: String) -> Result<()> {
        let filename = if Path::new(&filename).extension().is_some() {
            filename
        } else {
            format!("{filename}.csv")
        };

        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();

        if let Some(ref mut x) = *raw_data_recorder {
            if let Some(current_raw_data_file) = &x.current_raw_data_file {
                if current_raw_data_file.filename == filename {
                    x.current_raw_data_file = None;
                }
            }
        }

        let path = Path::new(&self.support_dir)
            .join("raw_data")
            .join(&filename);

        remove_file(&path)
            .with_context(|| format!("failed to remove raw data file: {}", path.display()))?;

        Ok(())
    }

    pub fn record_gps_data(
        &self,
        raw_data: &gps_processor::RawData,
        process_result: ProcessResult,
        received_timestamp_ms: i64,
    ) {
        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.record(raw_data, received_timestamp_ms);
        }
        drop(raw_data_recorder);

        let main_db = &mut self.dbs.lock().unwrap().0;
        main_db.record(raw_data, process_result).unwrap();
    }

    pub fn list_all_raw_data(&self) -> Vec<RawDataFile> {
        let dir = Path::new(&self.support_dir).join("raw_data");

        if !dir.exists() || !dir.is_dir() {
            return Vec::new();
        }

        let mut result: Vec<RawDataFile> = match std::fs::read_dir(&dir) {
            Result::Ok(entries) => entries
                .filter_map(|entry_res| {
                    let entry = entry_res.ok()?;
                    let path = entry.path();
                    if path.is_file() && path.extension()?.to_str()? == "csv" {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        Some(RawDataFile {
                            name,
                            path: path.to_string_lossy().to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        result.sort_by(|a, b| b.name.cmp(&a.name));
        result
    }

    pub fn set_finalized_journey_changed_callback(
        &mut self,
        callback: FinalizedJourneyChangedCallback,
    ) {
        self.finalized_journey_changed_callback = callback;
    }

    #[auto_context]
    pub fn get_latest_bitmap_for_main_map_renderer(
        &self,
        layer_kind: &Option<LayerKind>,
        include_ongoing: bool,
    ) -> Result<JourneyBitmap> {
        let mut dbs = self.dbs.lock().unwrap();
        let (ref mut main_db, ref cache_db) = *dbs;
        // passing `main_db` to `get_latest` directly is fine because it only reads `main_db`.
        let journey_bitmap =
            merged_journey_builder::get_latest(main_db, cache_db, layer_kind, include_ongoing)?;
        drop(dbs);

        Ok(journey_bitmap)
    }

    #[auto_context]
    pub fn clear_all_cache(&self) -> Result<()> {
        let cache_db = &self.dbs.lock().unwrap().1;
        cache_db.clear_all_cache()?;
        Ok(())
    }

    // TODO: do we need this?
    #[auto_context]
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
