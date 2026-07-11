extern crate simplelog;
use crate::achievement::{self, contract::AchievementReader, contract::AchievementStore};
use crate::cache_db::{self, CacheDb, LayerKind};
use crate::geo::GeoIndex;
use crate::gps_processor::{self, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_header::JourneyKind;
use crate::journey_snapshot::JourneySnapshot;
use crate::main_db::{self, Action, MainDb};
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fs::{remove_file, File};
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

impl RawCsvRow {
    pub fn create_from_raw_data(
        raw_data: &gps_processor::RawData,
        received_timestamp_ms: i64,
    ) -> Self {
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
        let row = RawCsvRow::create_from_raw_data(raw_data, received_timestamp_ms);
        current_raw_data_file.writer.serialize(row).unwrap();
        current_raw_data_file.writer.flush().unwrap();
    }
}

type FinalizedJourneyChangedCallback = Box<dyn Fn(&Storage) + Send + Sync + 'static>;

/// The three databases kept transactionally in sync behind one lock: the main
/// journey store, the bitmap cache, and the achievement store.
struct Dbs {
    main_db: MainDb,
    cache_db: Box<dyn CacheDb + Send>,
    achievement_store: Box<dyn AchievementStore + Send>,
}

pub struct Storage {
    support_dir: String,
    raw_data_recorder: Mutex<Option<RawDataRecorder>>, // `None` means disabled
    pub cache_dir: String,
    // Hidden so every operation goes through `Storage` and stays in sync; reads
    // via `with_journey_snapshot` / `with_achievement_read`. The achievement store
    // lives here, not in `MainState`, because granular invalidation needs the
    // precise `Action`, visible only inside `with_db_txn`.
    dbs: Mutex<Dbs>,
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
        let cache_db: Box<dyn CacheDb + Send> = Box::new(cache_db::new(&cache_dir));
        let achievement_store =
            achievement::new(&cache_dir).expect("failed to open achievement store");
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
            dbs: Mutex::new(Dbs {
                main_db,
                cache_db,
                achievement_store,
            }),
            finalized_journey_changed_callback: Box::new(|_| {}),
        }
    }

    #[auto_context]
    pub fn with_db_txn<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&mut main_db::Txn) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let Dbs {
            main_db,
            cache_db,
            achievement_store,
        } = &mut *dbs;

        let mut finalized_journey_changed = false;

        let output = main_db.with_txn(|txn| {
            let output = f(txn)?;

            match &txn.action {
                None => (),
                Some(action) => {
                    match action {
                        Action::CompleteRebuilt => {
                            cache_db.clear_all()?;
                        }
                        Action::Invalidate { entries } => {
                            cache_db.invalidate(entries)?;
                        }
                        Action::MergeOne { entry, data } => {
                            cache_db.merge_journey(entry, data)?;
                        }
                    };
                    // Any committed journey change invalidates the achievement
                    // store (recompute is deferred to the next read).
                    // TODO: Invalidate based on actions. Actions other than
                    //       MergeOne are rare.
                    achievement_store.invalidate_all()?;
                    finalized_journey_changed = true;
                }
            }

            Ok(output)
        })?;

        // Make sure we are not holding the lock when calling the callback
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
                let main_db = &mut self.dbs.lock().unwrap().main_db;
                main_db
                    .set_setting(crate::main_db::Setting::RawDataMode, true)
                    .unwrap();
            }
        } else if raw_data_recorder.is_some() {
            info!("[storage] raw data mod disabled");
            // `drop` should do the right thing and release all resources.
            *raw_data_recorder = None;
            let main_db = &mut self.dbs.lock().unwrap().main_db;
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

        let main_db = &mut self.dbs.lock().unwrap().main_db;
        main_db.record(raw_data, process_result).unwrap();
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

    /// Run `f` with a read-only [`JourneySnapshot`] under one `dbs` lock
    /// and one `MainDb` transaction. Every read `f` performs sees the
    /// SAME snapshot, so a journey merge cannot land between two reads
    /// and make them mutually inconsistent (e.g. an `All` bitmap smaller
    /// than `Default`'s). Callers compose whatever reads they need; the
    /// cache's mutating ops stay private to `Storage`, which owns the
    /// main_db↔cache_db sync invariant.
    ///
    /// Does NOT route through `with_db_txn` — `std::sync::Mutex` is not
    /// reentrant, so taking the `dbs` lock again would deadlock.
    #[auto_context]
    pub fn with_journey_snapshot<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&JourneySnapshot) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let Dbs {
            main_db, cache_db, ..
        } = &mut *dbs;
        main_db.with_txn(|txn| {
            let output = f(&JourneySnapshot::new(txn, cache_db.as_ref()))?;
            // The snapshot only exposes reads, so a journey action must
            // never have been recorded on this txn.
            debug_assert_eq!(txn.action, None);
            Ok(output)
        })
    }

    /// Run `f` against the up-to-date achievement store under one `dbs` lock and
    /// one read txn. The store opens a read over a single [`JourneySnapshot`]
    /// and computes on demand, so the values `f` reads are internally
    /// consistent. The store's mutating side
    /// stays private to `Storage`.
    ///
    /// Like `with_journey_snapshot`, does NOT route through `with_db_txn`
    /// (`std::sync::Mutex` is not reentrant).
    #[auto_context]
    pub fn with_achievement_read<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&dyn AchievementReader) -> Result<O>,
    {
        // TODO: locks here for now, add MVCC or background thread to recompute
        let mut dbs = self.dbs.lock().unwrap();
        let Dbs {
            main_db,
            cache_db,
            achievement_store,
        } = &mut *dbs;
        main_db.with_txn(|txn| {
            let snapshot = JourneySnapshot::new(txn, cache_db.as_ref());
            let reader = achievement_store.reader(&snapshot)?;
            let output = f(reader.as_ref())?;
            debug_assert_eq!(txn.action, None);
            Ok(output)
        })
    }

    /// Install a worldview's geo asset from raw bytes. The asset must declare
    /// the same worldview id it is loaded as (the `.bin` is self-describing); a
    /// mismatch means the wrong bin was supplied.
    #[auto_context]
    pub fn init_or_change_geo_data(
        &self,
        worldview: geo_data_format::Worldview,
        bytes: &[u8],
    ) -> Result<()> {
        let geo = GeoIndex::from_bytes(bytes)?;
        anyhow::ensure!(
            geo.worldview_id() == worldview.spec().id,
            "geo asset declares worldview {:?} but was loaded as {:?}",
            geo.worldview_id(),
            worldview.spec().id
        );
        let achievement_store = &mut self.dbs.lock().unwrap().achievement_store;
        achievement_store.set_geo(worldview, Box::new(geo))?;
        Ok(())
    }

    /// The bitmap the main map renders: finalized coverage for
    /// `layer_kind` (`None` → no finalized base) plus, when
    /// `include_ongoing`, the not-yet-finalized journey merged on top.
    #[auto_context]
    pub fn get_latest_bitmap_for_main_map_renderer(
        &self,
        layer_kind: &Option<LayerKind>,
        include_ongoing: bool,
    ) -> Result<JourneyBitmap> {
        self.with_journey_snapshot(|snapshot| {
            let mut bitmap = match layer_kind {
                Some(layer_kind) => snapshot.finalized_bitmap(layer_kind, None)?,
                None => JourneyBitmap::new(),
            };
            if include_ongoing {
                if let Some(journey_vector) = snapshot.ongoing_journey()? {
                    bitmap.merge_vector(&journey_vector);
                }
            }
            Ok(bitmap)
        })
    }

    /// Finalized coverage within `[from, to]`, optionally filtered to one
    /// journey kind (`None` → all kinds). Used by the time machine.
    #[auto_context]
    pub fn get_range_bitmap(
        &self,
        from_date_inclusive: NaiveDate,
        to_date_inclusive: NaiveDate,
        kind: Option<&JourneyKind>,
    ) -> Result<JourneyBitmap> {
        let layer_kind = match kind {
            Some(kind) => LayerKind::JourneyKind(*kind),
            None => LayerKind::All,
        };
        self.with_journey_snapshot(|snapshot| {
            snapshot.finalized_bitmap(&layer_kind, Some((from_date_inclusive, to_date_inclusive)))
        })
    }

    #[auto_context]
    pub fn clear_all_cache(&self) -> Result<()> {
        let cache_db = &self.dbs.lock().unwrap().cache_db;
        cache_db.clear_all()?;
        Ok(())
    }

    // TODO: do we need this?
    #[auto_context]
    pub fn _flush(&self) -> Result<()> {
        debug!("[storage] flushing");

        let dbs = self.dbs.lock().unwrap();
        dbs.main_db.flush()?;
        dbs.cache_db.flush()?;
        drop(dbs);

        let mut raw_data_recorder = self.raw_data_recorder.lock().unwrap();
        if let Some(ref mut x) = *raw_data_recorder {
            x.flush();
        }
        drop(raw_data_recorder);

        Ok(())
    }
}
