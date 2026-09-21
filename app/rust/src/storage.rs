extern crate simplelog;
use crate::achievement::AchievementReader;
use crate::cache_db::{self, CacheDb, LayerKind};
use crate::geo::{GeoAssetError, GeoIndex, GeoLookup};
use crate::gps_processor::ProcessResult;
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_header::JourneyKind;
use crate::journey_snapshot::JourneySnapshot;
use crate::legacy_raw_data::{self, LegacyRawDataFile};
use crate::main_db::{self, Action, MainDb};
use crate::raw_data::ExtendedRawGPSPoint;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::NaiveDate;
use std::fs::{remove_file, File};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

// TODO: error handling in this file is horrifying, we should think about what
// is the right thing to do here.

type FinalizedJourneyChangedCallback = Box<dyn Fn(&Storage) + Send + Sync + 'static>;

struct Inner {
    main_db: MainDb,
    cache_db: Box<dyn CacheDb + Send>,
    geo: Option<Box<dyn GeoLookup + Send>>,
    worldview: Option<geo_data_format::Worldview>,
}

fn geo_ref(geo: &Option<Box<dyn GeoLookup + Send>>) -> Option<&dyn GeoLookup> {
    geo.as_deref().map(|geo| geo as &dyn GeoLookup)
}

pub struct Storage {
    support_dir: String,
    raw_data_mode: AtomicBool,
    pub cache_dir: String,
    // Hidden so every operation goes through `Storage` and stays in sync; reads
    dbs: Mutex<Inner>,
    finalized_journey_changed_callback: FinalizedJourneyChangedCallback,
}

impl Storage {
    pub fn init(
        _temp_dir: String,
        _doc_dir: String,
        support_dir: String,
        cache_dir: String,
    ) -> Result<Self> {
        let mut main_db = MainDb::open(&support_dir)?;
        let cache_db: Box<dyn CacheDb + Send> = Box::new(cache_db::new(&cache_dir));
        let raw_data_mode =
            main_db.get_setting_with_default(crate::main_db::Setting::RawDataMode, false);
        Ok(Storage {
            support_dir,
            raw_data_mode: AtomicBool::new(raw_data_mode),
            cache_dir,
            dbs: Mutex::new(Inner {
                main_db,
                cache_db,
                geo: None,
                worldview: None,
            }),
            finalized_journey_changed_callback: Box::new(|_| {}),
        })
    }

    pub fn installed_geo_data_file(&self, worldview: geo_data_format::Worldview) -> PathBuf {
        Path::new(&self.support_dir)
            .join("geo")
            .join(format!("geo_data_{}.bin", worldview.spec().id))
    }

    #[auto_context]
    pub fn with_db_txn<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&mut main_db::Txn) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let Inner {
            main_db,
            cache_db,
            geo,
            ..
        } = &mut *dbs;
        let geo = geo_ref(geo);

        let mut finalized_journey_changed = false;
        let mut geo_broken = false;

        let output = main_db.with_txn(|txn| {
            let output = f(txn)?;

            if let Some(action) = &txn.action {
                match action {
                    Action::CompleteRebuilt => cache_db.clear_all()?,
                    Action::Invalidate { entries } => cache_db.invalidate(entries)?,
                    Action::MergeOne { entry, data, .. } => {
                        match cache_db.merge_journey(entry, data, geo) {
                            Err(e) if GeoAssetError::is_in(&e) => {
                                geo_broken = true;
                                cache_db.merge_journey(entry, data, None)?
                            }
                            merged => merged?,
                        }
                    }
                }
                finalized_journey_changed = true;
            }

            Ok(output)
        })?;
        if geo_broken {
            self.discard_geo(&mut dbs);
        }

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
        // Serialize setting changes with recording and finalization. Publish
        // the in-memory value only after the setting has been persisted.
        let main_db = &mut self.dbs.lock().unwrap().main_db;
        if self.get_raw_data_mode() != enable {
            main_db
                .set_setting(crate::main_db::Setting::RawDataMode, enable)
                .unwrap();
            self.raw_data_mode.store(enable, Ordering::Relaxed);
            info!("[storage] raw data mode enabled={enable}");
        }
    }

    pub fn get_raw_data_mode(&self) -> bool {
        self.raw_data_mode.load(Ordering::Relaxed)
    }

    #[auto_context]
    pub fn delete_legacy_raw_data_file(&self, filename: String) -> Result<()> {
        legacy_raw_data::delete_legacy_raw_data_file(&self.support_dir, &filename)
    }

    pub fn record_gps_data(&self, data: &ExtendedRawGPSPoint, process_result: ProcessResult) {
        let main_db = &mut self.dbs.lock().unwrap().main_db;
        main_db
            .record_with_raw_data(data, process_result, self.get_raw_data_mode())
            .unwrap();
    }

    pub fn list_all_legacy_raw_data(&self) -> Result<Vec<LegacyRawDataFile>> {
        legacy_raw_data::list_all_legacy_raw_data(&self.support_dir)
    }

    pub fn export_legacy_raw_data_gpx_file(&self, csv_filepath: &str) -> Result<String> {
        legacy_raw_data::export_legacy_raw_data_gpx_file(csv_filepath, &self.cache_dir)
    }

    pub fn set_finalized_journey_changed_callback(
        &mut self,
        callback: FinalizedJourneyChangedCallback,
    ) {
        self.finalized_journey_changed_callback = callback;
    }

    /// Run `f` with a logically read-only [`JourneySnapshot`] under one `dbs` lock
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
        F: FnOnce(&mut JourneySnapshot) -> Result<O>,
    {
        let mut dbs = self.dbs.lock().unwrap();
        let Inner {
            main_db, cache_db, ..
        } = &mut *dbs;
        main_db.with_txn(|txn| {
            let output = f(&mut JourneySnapshot::new(txn, cache_db.as_mut()))?;
            // The snapshot only exposes reads, so a journey action must
            // never have been recorded on this txn.
            debug_assert_eq!(txn.action, None);
            Ok(output)
        })
    }

    /// Run `f` against achievement answers under one `dbs` lock and one read
    /// txn, so the values `f` reads are internally consistent. Whether they come
    /// from persisted rows or are computed on the spot is the cache's business.
    ///
    /// Like `with_journey_snapshot`, does NOT route through `with_db_txn`
    /// (`std::sync::Mutex` is not reentrant).
    #[auto_context]
    pub fn with_achievement_read<F, O>(&self, f: F) -> Result<O>
    where
        F: FnOnce(&mut dyn AchievementReader) -> Result<O>,
    {
        // TODO: locks here for now, add MVCC or background thread to recompute
        let mut dbs = self.dbs.lock().unwrap();
        let Inner {
            main_db,
            cache_db,
            geo,
            ..
        } = &mut *dbs;
        let geo = geo_ref(geo);
        let cache_db = cache_db.as_mut();
        let output = main_db.with_txn(|txn| {
            let mut reader = cache_db.achievement_reader(txn, geo)?;
            let output = f(reader.as_mut())?;
            debug_assert_eq!(txn.action, None);
            Ok(output)
        });
        if output.as_ref().is_err_and(GeoAssetError::is_in) {
            self.discard_geo(&mut dbs);
        }
        output
    }

    fn discard_geo(&self, dbs: &mut Inner) {
        dbs.geo = None;
        if let Some(worldview) = dbs.worldview.take() {
            let path = self.installed_geo_data_file(worldview);
            warn!(
                "[storage] geo asset {} unreadable, discarding it for reinstall",
                path.display()
            );
            let _ = remove_file(path);
        }
    }

    #[auto_context]
    pub fn init_or_change_geo_data(
        &self,
        worldview: geo_data_format::Worldview,
        expected_provenance_hash: [u8; 32],
        bytes: &[u8],
    ) -> Result<()> {
        let path = self.installed_geo_data_file(worldview);
        let dir = path.parent().expect("installed geo path has a parent");
        std::fs::create_dir_all(dir)?;
        let tmp = path.with_extension("bin.tmp");
        {
            let mut file = File::create(&tmp)?;
            std::io::Write::write_all(&mut file, bytes)?;
            file.sync_all()?;
        }
        let staged = GeoIndex::open(&tmp).and_then(|geo| {
            anyhow::ensure!(
                geo.provenance_hash() == expected_provenance_hash,
                "geo asset provenance {:?} does not match the expected {:?}",
                geo.provenance_hash(),
                expected_provenance_hash
            );
            Self::check_worldview(&geo, worldview)
        });
        if let Err(e) = staged {
            let _ = remove_file(&tmp);
            return Err(e);
        }
        #[cfg(windows)]
        if path.exists() {
            remove_file(&path)?;
        }
        std::fs::rename(&tmp, &path)?;
        let geo = GeoIndex::open(&path)?;
        let mut dbs = self.dbs.lock().unwrap();
        dbs.geo = Some(Box::new(geo));
        dbs.worldview = Some(worldview);
        Ok(())
    }

    #[auto_context]
    pub fn open_installed_geo_data(
        &self,
        worldview: geo_data_format::Worldview,
        expected_provenance_hash: [u8; 32],
    ) -> Result<bool> {
        let path = self.installed_geo_data_file(worldview);
        if !path.exists() {
            return Ok(false);
        }
        let geo = match GeoIndex::open(&path) {
            std::result::Result::Ok(geo) => geo,
            Err(e) => {
                warn!(
                    "[storage] installed geo asset {} unusable, will reinstall: {e:#}",
                    path.display()
                );
                return Ok(false);
            }
        };
        if geo.provenance_hash() != expected_provenance_hash
            || Self::check_worldview(&geo, worldview).is_err()
        {
            return Ok(false);
        }
        let mut dbs = self.dbs.lock().unwrap();
        dbs.geo = Some(Box::new(geo));
        dbs.worldview = Some(worldview);
        Ok(true)
    }

    fn check_worldview(geo: &GeoIndex, worldview: geo_data_format::Worldview) -> Result<()> {
        anyhow::ensure!(
            geo.worldview_id() == worldview.spec().id,
            "geo asset declares worldview {:?} but was loaded as {:?}",
            geo.worldview_id(),
            worldview.spec().id
        );
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
        self.dbs.lock().unwrap().cache_db.clear_all()?;
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

        Ok(())
    }
}
