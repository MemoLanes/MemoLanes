use std::fs::File;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::gps_processor::{GpsProcessor, ProcessResult};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind};
use crate::map_renderer::{MapRenderer, RenderResult};
use crate::storage::Storage;
use crate::{archive, export_data, gps_processor, import_data, merged_journey_manager, storage};
use anyhow::{Ok, Result};
use chrono::Local;
use simplelog::{Config, LevelFilter, WriteLogger};

// TODO: we have way too many locking here and now it is hard to track.
//  e.g. we could mess up with the order and cause a deadlock
struct MainState {
    storage: Storage,
    map_renderer: Mutex<Option<MapRenderer>>,
    gps_processor: Mutex<GpsProcessor>,
}

static MAIN_STATE: OnceLock<MainState> = OnceLock::new();

pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) {
    let mut already_initialized = true;
    MAIN_STATE.get_or_init(|| {
        already_initialized = false;

        // init logging
        let path = Path::new(&cache_dir).join("main.log");
        WriteLogger::init(
            LevelFilter::Info,
            Config::default(),
            File::create(path).unwrap(),
        )
        .expect("Failed to initialize logging");

        let storage = Storage::init(temp_dir, doc_dir, support_dir, cache_dir);
        info!("initialized");

        MainState {
            storage,
            map_renderer: Mutex::new(None),
            gps_processor: Mutex::new(GpsProcessor::new()),
        }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

pub fn render_map_overlay(
    zoom: f32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> Option<RenderResult> {
    let state = get();
    let mut map_renderer = state.map_renderer.lock().unwrap();

    map_renderer
        .get_or_insert_with(|| {
            let mut main_db = state.storage.main_db.lock().unwrap();
            // TODO: error handling?
            let journey_bitmap =
                merged_journey_manager::get_latest_including_ongoing(&mut main_db).unwrap();
            MapRenderer::new(journey_bitmap)
        })
        .maybe_render_map_overlay(zoom, left, top, right, bottom)
}

pub fn reset_map_renderer() {
    let state = get();
    let mut map_renderer = state.map_renderer.lock().unwrap();

    if let Some(map_renderer) = &mut *map_renderer {
        map_renderer.reset();
    }
}

pub fn on_location_update(
    mut raw_data_list: Vec<gps_processor::RawData>,
    recevied_timestamp_ms: i64,
) {
    let state = get();
    // NOTE: On Android, we might recevied a batch of location updates that are out of order.
    // Not very sure why yet.

    // we need handle a batch in one go so we hold the lock for the whole time
    let mut gps_processor = state.gps_processor.lock().unwrap();
    let mut map_renderer = state.map_renderer.lock().unwrap();

    raw_data_list.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
    raw_data_list.into_iter().for_each(|raw_data| {
        // TODO: more batching updates
        gps_processor.preprocess(raw_data, |last_data, curr_data, process_result| {
            let line_to_add = match process_result {
                ProcessResult::Ignore => None,
                ProcessResult::NewSegment => Some((curr_data, curr_data)),
                ProcessResult::Append => {
                    let start = last_data.as_ref().unwrap_or(curr_data);
                    Some((start, curr_data))
                }
            };
            match map_renderer.as_mut() {
                None => (),
                Some(map_renderer) => match line_to_add {
                    None => (),
                    Some((start, end)) => {
                        map_renderer.update(|journey_bitmap| {
                            journey_bitmap.add_line(
                                start.longitude,
                                start.latitude,
                                end.longitude,
                                end.latitude,
                            );
                        });
                    }
                },
            }
            state
                .storage
                .record_gps_data(curr_data, process_result, recevied_timestamp_ms);
        });
    });
}

pub fn list_all_raw_data() -> Vec<storage::RawDataFile> {
    get().storage.list_all_raw_data()
}

pub fn get_raw_data_mode() -> bool {
    get().storage.get_raw_data_mode()
}

pub fn toggle_raw_data_mode(enable: bool) {
    get().storage.toggle_raw_data_mode(enable)
}

pub fn finalize_ongoing_journey() -> Result<bool> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    main_db.with_txn(|txn| txn.finalize_ongoing_journey())
}

pub fn try_auto_finalize_journy() -> Result<bool> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    main_db.try_auto_finalize_journy()
}

pub fn import_fow_data(zip_file_path: String) -> Result<()> {
    // TODO: This is really naive, mostly just a demo. We need to get real
    // values from users.
    let (journey_bitmap, _warnings) = import_data::load_fow_sync_data(&zip_file_path)?;
    let mut main_db = get().storage.main_db.lock().unwrap();
    main_db.with_txn(|txn| {
        txn.create_and_insert_journey(
            Local::now().date_naive(),
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Bitmap(journey_bitmap),
        )
    })?;
    Ok(())
}

pub fn list_all_journeys() -> Result<Vec<JourneyHeader>> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    main_db.with_txn(|txn| txn.list_all_journeys())
}

pub fn generate_full_archive(target_filepath: String) -> Result<()> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    let mut file = File::create(target_filepath)?;
    archive::archive_all_as_zip(&mut main_db, &mut file)?;
    drop(file);
    Ok(())
}

pub enum ExportType {
    GPX = 0,
    KML = 1,
}

pub fn export_journey(
    target_filepath: String,
    journey_id: String,
    export_type: ExportType,
) -> Result<()> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    let journey_data = main_db.with_txn(|txn| txn.get_journey(&journey_id))?;
    match journey_data {
        JourneyData::Bitmap(_bitmap) => Err(anyhow!("Data type error")),
        JourneyData::Vector(vector) => {
            let mut file = File::create(target_filepath)?;
            match export_type {
                ExportType::GPX => {
                    export_data::journey_vector_to_gpx_file(&vector, &mut file)?;
                }
                ExportType::KML => {
                    export_data::journey_vector_to_kml_file(&vector, &mut file)?;
                }
            }
            Ok(())
        }
    }
}

pub fn recover_from_archive(zip_file_path: String) -> Result<()> {
    let mut main_db = get().storage.main_db.lock().unwrap();
    archive::recover_archive_file(&zip_file_path, &mut main_db)?;
    Ok(())
}
