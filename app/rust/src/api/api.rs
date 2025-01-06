use std::fs::File;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Ok, Result};
use chrono::NaiveDate;
use flutter_rust_bridge::frb;

use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::journey_bitmap::{JourneyBitmap, MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET};
use crate::journey_data::JourneyData;
use crate::journey_header::JourneyHeader;
use crate::renderer::MapRenderer;
use crate::renderer::MapServer;
use crate::storage::Storage;
use crate::{archive, export_data, gps_processor, merged_journey_builder, storage};
use crate::{logs, utils};
use serde::{Deserialize, Serialize};

use super::import::JourneyInfo;

// TODO: we have way too many locking here and now it is hard to track.
//  e.g. we could mess up with the order and cause a deadlock
#[frb(ignore)]
pub(super) struct MainState {
    pub storage: Storage,
    pub map_renderer: Mutex<Option<MapRenderer>>,
    pub gps_preprocessor: Mutex<GpsPreprocessor>,
    pub map_server: Mutex<Option<MapServer>>,
}

static MAIN_STATE: OnceLock<MainState> = OnceLock::new();

#[frb(ignore)]
pub fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

#[frb(sync)]
pub fn short_commit_hash() -> String {
    env!("SHORT_COMMIT_HASH").to_string()
}

pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) {
    let mut already_initialized = true;
    MAIN_STATE.get_or_init(|| {
        already_initialized = false;

        // init logging
        logs::init(&cache_dir).expect("Failed to initialize logging");

        let storage = Storage::init(temp_dir, doc_dir, support_dir, cache_dir);
        info!("initialized");

        let mut map_server = MapServer::new("localhost", 0);
        map_server.start().unwrap();
        info!("map server started");

        // ======= WebView Transition codes START ===========
        // TODO: this is a temporary solution for WebView transition
        let journey_bitmap = storage.get_latest_bitmap_for_main_map_renderer().unwrap();
        let journey_bitmap = Arc::new(Mutex::new(journey_bitmap));
        map_server.set_journey_bitmap_with_poll_handler(
            Arc::downgrade(&journey_bitmap),
            Some(Box::new(poll_for_main_map_update)),
        );
        let map_renderer = MapRenderer::debug_new(journey_bitmap);
        // ======= WebView Transition codes END ===========

        MainState {
            storage,
            map_renderer: Mutex::new(Some(map_renderer)),
            gps_preprocessor: Mutex::new(GpsPreprocessor::new()),
            map_server: Mutex::new(Some(map_server)),
        }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

#[frb(opaque)]
pub enum MapRendererProxy {
    MainMap,
    Simple(MapRenderer),
}

#[frb(ignore)]
pub fn poll_for_main_map_update(journey_bitmap: &mut JourneyBitmap) -> bool {
    let state = get();
    let need_reload = state.storage.main_map_renderer_need_to_reload();
    if need_reload {
        *journey_bitmap = state
            .storage
            .get_latest_bitmap_for_main_map_renderer()
            .unwrap();
    }
    need_reload
}

#[frb(sync)]
pub fn get_url() -> String {
    let state = get();
    state.map_server.lock().unwrap().as_ref().unwrap().get_url()
}

#[frb(sync)]
pub fn get_map_renderer_proxy_for_main_map() -> MapRendererProxy {
    // ======= WebView Transition codes START ===========
    // We need to reactivate the main map
    // TODO: this is a temporary solution for WebView transition
    let journey_bitmap = get()
        .map_renderer
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .debug_get_journey_bitmap();
    get()
        .map_server
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .set_journey_bitmap_with_poll_handler(
            Arc::downgrade(&journey_bitmap),
            Some(Box::new(poll_for_main_map_update)),
        );
    // ======= WebView Transition codes END ===========

    MapRendererProxy::MainMap
}

#[frb(sync)]
pub fn get_empty_map_renderer_proxy() -> MapRendererProxy {
    let journey_bitmap = JourneyBitmap::new();
    let map_renderer = MapRenderer::new(journey_bitmap);
    MapRendererProxy::Simple(map_renderer)
}

pub fn get_map_renderer_proxy_for_journey_date_range(
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
) -> Result<MapRendererProxy> {
    let journey_bitmap = get().storage.with_db_txn(|txn| {
        merged_journey_builder::get_range(txn, from_date_inclusive, to_date_inclusive)
    })?;

    // ======= WebView Transition codes START ===========
    let journey_bitmap = Arc::new(Mutex::new(journey_bitmap));
    let server = get().map_server.lock().unwrap();
    server
        .as_ref()
        .unwrap()
        .set_journey_bitmap_with_poll_handler(Arc::downgrade(&journey_bitmap), None);
    let map_renderer = MapRenderer::debug_new(journey_bitmap);
    // ======= WebView Transition codes END ===========

    Ok(MapRendererProxy::Simple(map_renderer))
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CameraOption {
    pub zoom: f64,
    pub lng: f64,
    pub lat: f64,
}

// TODO: redesign this interface at a better position
pub(crate) fn get_default_camera_option_from_journey_bitmap(
    journey_bitmap: &JourneyBitmap,
) -> Option<CameraOption> {
    // TODO: Currently we use the coordinate of the top left of a random block (first one in the hashtbl),
    // then just pick a hardcoded zoom level.
    // A better version could be finding a bounding box (need to be careful with the antimeridian).
    journey_bitmap
        .tiles
        .iter()
        .next()
        .and_then(|(tile_pos, tile)| {
            // we shouldn't have empty tile or block
            tile.blocks.keys().next().map(|block_pos| {
                let blockzoomed_x: i32 = TILE_WIDTH as i32 * tile_pos.0 as i32 + block_pos.0 as i32;
                let blockzoomed_y: i32 = TILE_WIDTH as i32 * tile_pos.1 as i32 + block_pos.1 as i32;
                let (lng, lat) = utils::tile_x_y_to_lng_lat(
                    blockzoomed_x,
                    blockzoomed_y,
                    (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32,
                );
                CameraOption {
                    zoom: 12.0,
                    lng,
                    lat,
                }
            })
        })
}

pub fn get_map_renderer_proxy_for_journey(
    journey_id: &str,
) -> Result<(MapRendererProxy, Option<CameraOption>)> {
    let journey_data = get()
        .storage
        .with_db_txn(|txn| txn.get_journey(journey_id))?;

    let journey_bitmap = match journey_data {
        JourneyData::Bitmap(bitmap) => bitmap,
        JourneyData::Vector(vector) => {
            let mut bitmap = JourneyBitmap::new();
            merged_journey_builder::add_journey_vector_to_journey_bitmap(&mut bitmap, &vector);
            bitmap
        }
    };

    let default_camera_option = get_default_camera_option_from_journey_bitmap(&journey_bitmap);

    // ======= WebView Transition codes START ===========
    let journey_bitmap = Arc::new(Mutex::new(journey_bitmap));
    let server = get().map_server.lock().unwrap();
    server
        .as_ref()
        .unwrap()
        .set_journey_bitmap_with_poll_handler(Arc::downgrade(&journey_bitmap), None);
    server
        .as_ref()
        .unwrap()
        .set_provisioned_camera_option(default_camera_option);
    let map_renderer = MapRenderer::debug_new(journey_bitmap);
    // ======= WebView Transition codes END ===========

    Ok((
        MapRendererProxy::Simple(map_renderer),
        default_camera_option,
    ))
}

pub fn on_location_update(
    mut raw_data_list: Vec<gps_processor::RawData>,
    recevied_timestamp_ms: i64,
) {
    let state = get();
    // NOTE: On Android, we might recevied a batch of location updates that are out of order.
    // Not very sure why yet.

    // we need handle a batch in one go so we hold the lock for the whole time
    let mut gps_preprocessor = state.gps_preprocessor.lock().unwrap();
    let mut map_renderer = state.map_renderer.lock().unwrap();

    raw_data_list.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
    raw_data_list.into_iter().for_each(|raw_data| {
        // TODO: more batching updates
        let last_point = gps_preprocessor.last_point();
        let process_result = gps_preprocessor.preprocess(&raw_data);
        let line_to_add = match process_result {
            ProcessResult::Ignore => None,
            ProcessResult::NewSegment => Some((&raw_data.point, &raw_data.point)),
            ProcessResult::Append => {
                let start = last_point.as_ref().unwrap_or(&raw_data.point);
                Some((start, &raw_data.point))
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
                    // TODO: in current design, the active map_server data may not be the main map
                    state
                        .map_server
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .set_needs_reload();
                }
            },
        }
        state
            .storage
            .record_gps_data(&raw_data, process_result, recevied_timestamp_ms);
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

pub fn delete_journey(journey_id: &str) -> Result<()> {
    get()
        .storage
        .with_db_txn(|txn| txn.delete_journey(journey_id))
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

pub fn has_ongoing_journey() -> Result<bool> {
    Ok(get()
        .storage
        .with_db_txn(|txn| txn.get_lastest_timestamp_of_ongoing_journey())?
        .is_some())
}

pub fn years_with_journey() -> Result<Vec<i32>> {
    get().storage.with_db_txn(|txn| txn.years_with_journey())
}

pub fn months_with_journey(year: i32) -> Result<Vec<i32>> {
    get()
        .storage
        .with_db_txn(|txn| txn.months_with_journey(year))
}

pub fn days_with_journey(year: i32, month: i32) -> Result<Vec<i32>> {
    get()
        .storage
        .with_db_txn(|txn| txn.days_with_journey(year, month))
}

pub fn list_journy_on_date(year: i32, month: u32, day: u32) -> Result<Vec<JourneyHeader>> {
    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
    get()
        .storage
        .with_db_txn(|txn| txn.query_journeys(Some(date), Some(date)))
}

pub fn list_all_journeys() -> Result<Vec<JourneyHeader>> {
    get()
        .storage
        .with_db_txn(|txn| txn.query_journeys(None, None))
}

pub fn generate_full_archive(target_filepath: String) -> Result<()> {
    info!("generating full archive");
    let mut file = File::create(target_filepath)?;
    get()
        .storage
        .with_db_txn(|txn| archive::export_as_mldx(&archive::WhatToExport::All, txn, &mut file))?;
    drop(file);
    Ok(())
}

pub fn generate_single_archive(journey_id: String, target_filepath: String) -> Result<()> {
    info!("generating single journey archive");
    let mut file = File::create(target_filepath)?;
    get().storage.with_db_txn(|txn| {
        archive::export_as_mldx(&archive::WhatToExport::Just(journey_id), txn, &mut file)
    })?;
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

pub fn delete_all_journeys() -> Result<()> {
    info!("Delete all journeys");
    get().storage.with_db_txn(|txn| txn.delete_all_journeys())
}

pub fn import_archive(mldx_file_path: String) -> Result<()> {
    info!("Import Archived Data");
    get()
        .storage
        .with_db_txn(|txn| archive::import_mldx(txn, &mldx_file_path))?;
    Ok(())
}

pub fn update_journey_metadata(id: &str, journeyinfo: JourneyInfo) -> Result<()> {
    get().storage.with_db_txn(|txn| {
        txn.update_journey_metadata(
            id,
            journeyinfo.journey_date,
            journeyinfo.start_time,
            journeyinfo.end_time,
            journeyinfo.note,
        )
    })?;
    Ok(())
}

#[derive(Debug)]
pub struct DeviceInfo {
    pub is_physical_device: bool,
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

pub fn earliest_journey_date() -> Result<Option<NaiveDate>> {
    get().storage.with_db_txn(|txn| txn.earliest_journey_date())
}

pub fn export_logs(target_file_path: String) -> Result<()> {
    logs::export(&get().storage.cache_dir, &target_file_path)?;
    Ok(())
}

pub fn ten_minutes_heartbeat() {
    info!("10 minutes heartbeat");
}
