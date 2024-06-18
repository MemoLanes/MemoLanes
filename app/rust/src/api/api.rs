use std::cmp::max;
use std::fs::File;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Ok, Result};
use flutter_rust_bridge::frb;
use simplelog::{Config, LevelFilter, WriteLogger};

use crate::gps_processor::{GpsProcessor, ProcessResult};
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyHeader;
use crate::map_renderer::{MapRenderer, RenderResult};
use crate::storage::Storage;
use crate::{archive, export_data, gps_processor, storage};

// TODO: we have way too many locking here and now it is hard to track.
//  e.g. we could mess up with the order and cause a deadlock
#[frb(ignore)]
pub struct MainState {
    pub storage: Storage,
    pub map_renderer: Mutex<Option<MapRenderer>>,
    pub gps_processor: Mutex<GpsProcessor>,
}

static MAIN_STATE: OnceLock<MainState> = OnceLock::new();

#[frb(sync)]
pub fn short_commit_hash() -> String {
    env!("SHORT_COMMIT_HASH").to_string()
}

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

#[frb(ignore)]
pub fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

pub fn render_map_overlay(
    zoom: f32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> Option<RenderResult> {
    // TODO: right now the quality of zoom = 1 is really bad.
    let zoom = max(zoom as i32, 2);
    let state = get();
    let mut map_renderer = state.map_renderer.lock().unwrap();
    if state.storage.main_map_renderer_need_to_reload() {
        *map_renderer = None;
    }

    map_renderer
        .get_or_insert_with(|| {
            // TODO: error handling?
            let journey_bitmap = state
                .storage
                .get_latest_bitmap_for_main_map_renderer()
                .unwrap();
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

pub fn delete_raw_data_file(filename: String) -> Result<()> {
    get().storage.delete_raw_data_file(filename)
}

pub fn toggle_raw_data_mode(enable: bool) {
    get().storage.toggle_raw_data_mode(enable)
}

pub fn finalize_ongoing_journey() -> Result<bool> {
    get()
        .storage
        .with_db_txn(|txn| txn.finalize_ongoing_journey())
}

pub fn try_auto_finalize_journy() -> Result<bool> {
    get()
        .storage
        .with_db_txn(|txn| txn.try_auto_finalize_journy())
}

// fn convert_vec_to_nested_iterator(
//     data_segments: Vec<Result<RawData>>,
// ) -> impl Iterator<Item = impl Iterator<Item = Result<RawData>>> {
//     data_segments
//         .into_iter()
//         .map(|raw_data_result| raw_data_result.map(|raw_data| Ok(raw_data)).into_iter())
// }

// pub fn save_import_journey(journey_info: JourneyInfo, run_preprocessor: bool) -> Result<()> {
//     let journey_date = journey_info.journey_date.date_naive();
//     let rawdata = match journey_info.data {
//         ReadData::RawData(rawdata) => {
//             let journey_vector: JourneyVector =
//                 import_data::load_vector_data(rawdata.into_iter(), run_preprocessor)?;
//         }
//         ReadData::JourneyData(_) => todo!(),
//     };

//     // let journey_vector: JourneyVector = import_data::load_vector_data(journey_info.data, run_preprocessor)?;

//     let journey_data: JourneyData = JourneyData::Vector(journey_vector);
//     get().storage.with_db_txn(|txn| {
//         txn.create_and_insert_journey(
//             journey_date,
//             journey_info.start_time,
//             journey_info.end_time,
//             None,
//             JourneyKind::DefaultKind,
//             journey_info.note,
//             journey_data,
//         )
//     })?;
//     Ok(())
// }

// pub fn read_import_data(file_path: String, import_type: ImportType) -> Result<JourneyInfo> {
//     let journey_info = match import_type {
//         ImportType::GPX => import_data::load_gpx(&file_path)?,
//         ImportType::KML => import_data::load_kml(&file_path)?,
//         ImportType::FOW => {
//             let (journey_bitmap, _warnings) = import_data::load_fow_sync_data(&file_path)?;
//             JourneyInfo {
//                 data: ReadData::JourneyData(journey_bitmap),
//                 journey_date: Utc::now(),
//                 start_time: None,
//                 end_time: None,
//                 journey_header: None,
//                 note: None,
//             }
//         }
//     };
//     Ok(journey_info)
// }

pub fn list_all_journeys() -> Result<Vec<JourneyHeader>> {
    get().storage.with_db_txn(|txn| txn.list_all_journeys())
}

pub fn generate_full_archive(target_filepath: String) -> Result<()> {
    let mut file = File::create(target_filepath)?;
    get()
        .storage
        .with_db_txn(|txn| archive::archive_all_as_zip(txn, &mut file))?;
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
    let journey_data = get()
        .storage
        .with_db_txn(|txn| txn.get_journey(&journey_id))?;
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
    get()
        .storage
        .with_db_txn(|txn| archive::recover_archive_file(txn, &zip_file_path))?;
    Ok(())
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub system_version: Option<String>,
}

#[derive(Debug)]
pub struct AppInfo {
    pub package_name: String,
    pub version: String,
    pub build_number: String,
}

pub fn delayed_init(device_info: &DeviceInfo, app_info: &AppInfo) {
    info!(
        "[delayedInit] {:?}, {:?}, commit_hash = {}",
        device_info,
        app_info,
        short_commit_hash()
    );
}
