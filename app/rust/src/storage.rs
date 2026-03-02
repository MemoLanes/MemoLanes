extern crate simplelog;
use crate::cache_db::{CacheDb, LayerKind};
use crate::export_data;
use crate::gps_processor::{self, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyKind;
use crate::main_db::{self, Action, MainDb};
use crate::merged_journey_builder;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use std::collections::HashMap;
use std::fs::remove_file;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

// TODO: error handling in this file is horrifying, we should think about what
// is the right thing to do here.

pub struct RawDataFile {
    pub name: String,
    pub path: String,
}

pub use gps_processor::JourneyRawDataPoint as RawCsvRow;

type FinalizedJourneyChangedCallback = Box<dyn Fn(&Storage) + Send + Sync + 'static>;

pub struct Storage {
    support_dir: String,
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
        let main_db = MainDb::open(&support_dir);
        let cache_db = CacheDb::open(&cache_dir);
        Storage {
            support_dir,
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

    pub fn toggle_raw_data_mode(&self, enable: bool) -> bool {
        let has_ongoing = self
            .with_db_txn(|txn| txn.get_ongoing_journey_timestamp_range())
            .map(|o| o.is_some())
            .unwrap_or(false);
        if has_ongoing {
            return false;
        }
        let mut dbs = self.dbs.lock().unwrap();
        let main_db = &mut dbs.0;
        let current = main_db.get_setting_with_default(crate::main_db::Setting::RawDataMode, false);
        if current == enable {
            return false;
        }
        main_db
            .set_setting(crate::main_db::Setting::RawDataMode, enable)
            .unwrap();
        if enable {
            info!("[storage] raw data mode enabled");
        } else {
            info!("[storage] raw data mode disabled");
        }
        true
    }

    pub fn get_raw_data_mode(&self) -> bool {
        let mut dbs = self.dbs.lock().unwrap();
        dbs.0
            .get_setting_with_default(crate::main_db::Setting::RawDataMode, false)
    }

    #[auto_context]
    pub fn delete_raw_data_file(&self, filename: String) -> Result<()> {
        let filename = if Path::new(&filename).extension().is_some() {
            filename
        } else {
            format!("{filename}.csv")
        };

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
        let mut dbs = self.dbs.lock().unwrap();
        let main_db = &mut dbs.0;
        let raw_data_mode =
            main_db.get_setting_with_default(crate::main_db::Setting::RawDataMode, false);
        main_db
            .record(
                raw_data,
                process_result,
                received_timestamp_ms,
                raw_data_mode,
            )
            .unwrap();
    }

    pub fn list_all_raw_data(&self) -> Result<Vec<RawDataFile>> {
        let dir = Path::new(&self.support_dir).join("raw_data");

        if !dir.exists() {
            return Ok(Vec::new());
        }

        if !dir.is_dir() {
            anyhow::bail!("raw_data path exists but is not a directory: {dir:?}");
        }

        let mut result: Vec<RawDataFile> = std::fs::read_dir(&dir)?
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
            .collect();

        result.sort_by(|a, b| b.name.cmp(&a.name));
        Ok(result)
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
        Ok(())
    }

    #[auto_context]
    pub fn export_journey_raw_data_csv(
        &self,
        journey_id: &str,
        target_filepath: &Path,
    ) -> Result<()> {
        let points = self
            .with_db_txn(|txn| {
                txn.get_journey_raw_data(journey_id)?
                    .map(|rd| rd.as_points())
                    .transpose()
            })?
            .ok_or_else(|| anyhow::anyhow!("Journey has no raw data"))?;
        export_data::raw_data_points_to_csv_file(target_filepath, &points)
            .context("export_journey_raw_data_csv")?;
        Ok(())
    }

    #[auto_context]
    pub fn export_journey_raw_data_gpx(
        &self,
        journey_id: &str,
        target_filepath: &Path,
    ) -> Result<()> {
        let points = self
            .with_db_txn(|txn| {
                txn.get_journey_raw_data(journey_id)?
                    .map(|rd| rd.as_points())
                    .transpose()
            })?
            .ok_or_else(|| anyhow::anyhow!("Journey has no raw data"))?;
        let mut file = File::create(target_filepath)
            .with_context(|| format!("Failed to create GPX file: {}", target_filepath.display()))?;
        let mut writer = BufWriter::new(&mut file);
        export_data::raw_data_points_to_gpx_file(&points, &mut writer)
            .context("export_journey_raw_data_gpx")?;
        writer.flush()?;
        Ok(())
    }
}
