extern crate simplelog;
use crate::achievement::AchievementReader;
use crate::api::achievement::RegionGeometry;
use crate::cache_db::{self, CacheDb, LayerKind};
use crate::display_geometry;
use crate::geo::{GeoAssetError, GeoIndex, GeoLookup};
use crate::gps::ExtendedRawGPSPoint;
use crate::gps_processor::ProcessResult;
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_header::JourneyKind;
use crate::journey_snapshot::JourneySnapshot;
use crate::legacy_raw_data::{self, LegacyRawDataFile};
use crate::main_db::{self, Action, MainDb};
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::NaiveDate;
use geo_data_format::{GeoData, GeoEntityId};
use std::fs::{remove_file, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

// TODO: error handling in this file is horrifying, we should think about what
// is the right thing to do here.

type FinalizedJourneyChangedCallback = Box<dyn Fn(&Storage) + Send + Sync + 'static>;

static GEO_INSTALL: Mutex<()> = Mutex::new(());

fn display_repair_marker(installed: &Path) -> PathBuf {
    installed.with_extension("bin.repair")
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => std::result::Result::Ok(()),
        result => result,
    }
}

struct Inner {
    main_db: MainDb,
    cache_db: Box<dyn CacheDb + Send>,
    geo: Option<Arc<GeoIndex>>,
    worldview: Option<geo_data_format::Worldview>,
}

fn geo_ref(geo: &Option<Arc<GeoIndex>>) -> Option<&dyn GeoLookup> {
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
        let storage = Storage {
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
        };
        storage.clean_geo_staging_files();
        Ok(storage)
    }

    fn clean_geo_staging_files(&self) {
        let _install = GEO_INSTALL.lock().unwrap();
        for &worldview in geo_data_format::Worldview::ALL {
            let path = self
                .installed_geo_data_file(worldview)
                .with_extension("bin.tmp");
            if let Err(error) = remove_if_present(&path) {
                warn!("[storage] could not remove {}: {error}", path.display());
            }
        }
    }

    pub fn region_geometry(&self, id: GeoEntityId) -> Result<Option<RegionGeometry>> {
        let (worldview, geo) = {
            let dbs = self.dbs.lock().unwrap();
            (
                dbs.worldview.context("no active geo worldview")?,
                dbs.geo.clone().context("no active geo data")?,
            )
        };
        let display = geo.display_geometry(id);
        if let Err(error) = &display {
            self.mark_display_for_repair(worldview, id)
                .with_context(|| {
                    format!(
                        "recording display repair after reading ID {} failed: {error:#}",
                        id.0
                    )
                })?;
        }
        let geometry = display.and_then(|geometry| {
            geometry
                .map(|geometry| display_geometry::feature(id, worldview.into(), geometry))
                .transpose()
        });
        let dbs = self.dbs.lock().unwrap();
        anyhow::ensure!(
            dbs.geo
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, &geo)),
            "active geo data changed during display lookup"
        );
        geometry
    }

    fn mark_display_for_repair(
        &self,
        worldview: geo_data_format::Worldview,
        id: GeoEntityId,
    ) -> Result<()> {
        let _install = GEO_INSTALL.lock().unwrap();
        let path = self.installed_geo_data_file(worldview);
        let marker = display_repair_marker(&path);
        // A reader can hold an old file across replacement. Confirm the failure
        // against the installed copy before recording a durable repair request.
        if !marker.try_exists()?
            && path.try_exists()?
            && GeoIndex::open_installed(&path)
                .and_then(|geo| geo.display_geometry(id))
                .is_err()
        {
            File::create_new(&marker)?.sync_all()?;
        }
        Ok(())
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

    /// Sample the retention setting under the same lock as recording and mode
    /// changes. Finalization also updates caches and notifies listeners.
    #[auto_context]
    pub fn finalize_ongoing_journey(&self) -> Result<bool> {
        self.with_db_txn(|txn| txn.finalize_ongoing_journey(self.get_raw_data_mode()))
    }

    #[auto_context]
    pub fn try_auto_finalize_journey(&self) -> Result<bool> {
        self.with_db_txn(|txn| txn.try_auto_finalize_journey(self.get_raw_data_mode()))
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
        let _install = GEO_INSTALL.lock().unwrap();
        let path = self.installed_geo_data_file(worldview);
        let repair_marker = display_repair_marker(&path);
        let repairing = repair_marker.try_exists()?;
        let staged = path.with_extension("bin.tmp");
        std::fs::create_dir_all(path.parent().expect("installed geo path has a parent"))?;
        remove_if_present(&staged)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            let data = GeoData::open(&staged)?;
            if repairing {
                data.validate_display_parity()?;
                data.validate_display(|_, _| Ok(()))
                    .context("checking display frames before replacing damaged installation")?;
            }
            let geo = GeoIndex::from_data(data)?;
            anyhow::ensure!(
                geo.provenance_hash() == expected_provenance_hash,
                "geo asset provenance {:?} does not match the expected {:?}",
                geo.provenance_hash(),
                expected_provenance_hash
            );
            Self::check_worldview(&geo, worldview)?;
            // The old index cannot discard the newly published file before the swap.
            let mut dbs = self.dbs.lock().unwrap();
            std::fs::rename(&staged, &path)?;
            if repairing {
                remove_if_present(&repair_marker)?;
            }
            dbs.geo = Some(Arc::new(geo));
            dbs.worldview = Some(worldview);
            Ok(())
        })();
        if result.is_err() {
            if let Err(error) = remove_if_present(&staged) {
                warn!("[storage] could not remove {}: {error}", staged.display());
            }
        }
        result
    }

    #[auto_context]
    pub fn open_installed_geo_data(
        &self,
        worldview: geo_data_format::Worldview,
        expected_provenance_hash: [u8; 32],
    ) -> Result<bool> {
        let _install = GEO_INSTALL.lock().unwrap();
        let mut dbs = self.dbs.lock().unwrap();
        let path = self.installed_geo_data_file(worldview);
        let needs_repair = display_repair_marker(&path).try_exists()?;
        if needs_repair && dbs.geo.is_some() {
            return Ok(false);
        }
        if !path.try_exists()? {
            return Ok(false);
        }
        let geo = match GeoIndex::open_installed(&path) {
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
        dbs.geo = Some(Arc::new(geo));
        dbs.worldview = Some(worldview);
        // On a cold start a known damaged display can still serve coverage
        // while the caller obtains the replacement from the bundle.
        Ok(!needs_repair)
    }

    pub(crate) fn has_coverage_during_display_repair(
        &self,
        worldview: geo_data_format::Worldview,
        provenance: [u8; 32],
    ) -> bool {
        let dbs = self.dbs.lock().unwrap();
        dbs.worldview == Some(worldview)
            && dbs
                .geo
                .as_ref()
                .is_some_and(|geo| geo.provenance_hash() == provenance)
            && display_repair_marker(&self.installed_geo_data_file(worldview)).is_file()
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
                    bitmap.edit().merge_vector(&journey_vector);
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
