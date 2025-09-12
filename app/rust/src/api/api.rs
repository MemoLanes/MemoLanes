use std::fs::File;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use chrono::NaiveDate;
use flutter_rust_bridge::frb;

use crate::cache_db::LayerKind as InternalLayerKind;
use crate::frb_generated::StreamSink;
use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::journey_bitmap::{JourneyBitmap, MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET};
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};
use crate::renderer::internal_server::Request;
use crate::renderer::map_server::MapRendererToken;
use crate::renderer::MapRenderer;
use crate::renderer::MapServer;
use crate::storage::Storage;
use crate::{
    archive, build_info, export_data, gps_processor, main_db, merged_journey_builder, storage,
};
use crate::{logs, utils};
use serde::{Deserialize, Serialize};

use super::import::JourneyInfo;

use log::{error, info, warn};

// TODO: we have way too many locking here and now it is hard to track.
//  e.g. we could mess up with the order and cause a deadlock
#[frb(ignore)]
pub(super) struct MainState {
    pub storage: Storage,
    pub gps_preprocessor: Mutex<GpsPreprocessor>,
    pub map_server: Mutex<MapServer>,
    // TODO: we should reconsider the way we handle the main map
    pub main_map_layer_kind: Arc<Mutex<InternalLayerKind>>,
    pub main_map_renderer: Arc<Mutex<MapRenderer>>,
    pub main_map_renderer_token: MapRendererToken,
}

static MAIN_STATE: OnceLock<MainState> = OnceLock::new();

#[frb(ignore)]
pub fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

#[frb(sync)]
pub fn short_commit_hash() -> String {
    build_info::SHORT_COMMIT_HASH.to_string()
}

pub fn init(temp_dir: String, doc_dir: String, support_dir: String, cache_dir: String) {
    let mut already_initialized = true;
    MAIN_STATE.get_or_init(|| {
        already_initialized = false;

        // init logging
        logs::init(&cache_dir).expect("Failed to initialize logging");

        let mut storage = Storage::init(temp_dir, doc_dir, support_dir, cache_dir);
        info!("initialized");

        let mut map_server =
            MapServer::create_and_start("localhost", None).expect("Failed to start map server");
        info!("map server started");

        let default_layer_kind = InternalLayerKind::JounreyKind(JourneyKind::DefaultKind);
        let main_map_layer_kind = Arc::new(Mutex::new(default_layer_kind));
        let main_map_layer_kind_copy = main_map_layer_kind.clone();
        // TODO: use an empty journey bitmap first, because loading could be slow (especially when we don't have cache).
        // Ideally, we should support main map renderer being none. e.g. we free it when the user is not using the map.
        let main_map_renderer = Arc::new(Mutex::new(MapRenderer::new(JourneyBitmap::new())));
        let main_map_renderer_copy = main_map_renderer.clone();
        // TODO: redesign the callback to better handle locks and avoid deadlocks
        storage.set_finalized_journey_changed_callback(Box::new(move |storage| {
            let mut map_renderer = main_map_renderer_copy.lock().unwrap();
            let layer_kind = main_map_layer_kind_copy.lock().unwrap();
            match storage.get_latest_bitmap_for_main_map_renderer(&layer_kind) {
                Err(e) => {
                    error!("Failed to get latest bitmap for main map renderer: {e:?}");
                }
                Ok(journey_bitmap) => {
                    map_renderer.replace(journey_bitmap);
                }
            }
        }));
        let main_map_renderer_token = map_server.register_map_renderer(main_map_renderer.clone());
        info!("main map renderer initialized");

        MainState {
            storage,
            gps_preprocessor: Mutex::new(GpsPreprocessor::new()),
            map_server: Mutex::new(map_server),
            main_map_layer_kind,
            main_map_renderer,
            main_map_renderer_token,
        }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

// TODO: this design is not ideal, we need this becuase the `init` above uses an empty one.
pub fn init_main_map() -> Result<()> {
    let state = get();
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let layer_kind = state.main_map_layer_kind.lock().unwrap();
    let journey_bitmap = state
        .storage
        .get_latest_bitmap_for_main_map_renderer(&layer_kind)?;
    map_renderer.replace(journey_bitmap);
    Ok(())
}

pub fn subscribe_to_log_stream(sink: StreamSink<String>) -> Result<()> {
    let mut logger = logs::FLUTTER_LOGGER.lock().unwrap();
    let old_sink = logger.take();
    *logger = Some(sink);
    // NOTE: The following code is important for flutter hot restart. We need to
    // release the `logger` lock before freeing the `old_sink`, otherwise
    // there will be a deadlock.
    drop(logger);
    let _ = old_sink;
    Ok(())
}

#[frb(sync)]
pub fn write_log(message: String, level: LogLevel) {
    match level {
        LogLevel::Info => info!("[Flutter] {}", message),
        LogLevel::Warn => warn!("[Flutter] {}", message),
        LogLevel::Error => error!("[Flutter] {}", message),
    }
}

#[derive(Debug)]
pub enum LogLevel {
    Info = 0,
    Warn = 1,
    Error = 2,
}

#[frb(opaque)]
pub enum MapRendererProxy {
    Token(MapRendererToken),
}

impl MapRendererProxy {
    #[frb(sync)]
    pub fn get_url(&self) -> String {
        match self {
            MapRendererProxy::Token(token) => token.url_hash_params(),
        }
    }

    #[frb(sync)]
    pub fn get_url_endpoint(&self) -> String {
        match self {
            MapRendererProxy::Token(token) => token.url_hash_params(),
        }
    }

    #[frb(sync)]
    pub fn get_journey_id(&self) -> String {
        match self {
            MapRendererProxy::Token(token) => token.journey_id(),
        }
    }
}

#[frb(sync)]
pub fn get_map_renderer_proxy_for_main_map() -> MapRendererProxy {
    let token = get().main_map_renderer_token.clone_temporary_token();

    MapRendererProxy::Token(token)
}

// TODO: does this interface necessary?
#[frb(sync)]
pub fn get_empty_map_renderer_proxy() -> MapRendererProxy {
    let state = get();

    let journey_bitmap = JourneyBitmap::new();

    let mut server = state.map_server.lock().unwrap();
    let map_renderer = MapRenderer::new(journey_bitmap);
    let token = server.register_map_renderer(Arc::new(Mutex::new(map_renderer)));
    MapRendererProxy::Token(token)
}

#[frb(sync)]
pub fn get_server_ipc_test_url() -> String {
    let state = get();
    let server = state.map_server.lock().unwrap();
    server.get_ipc_test_url()
}

pub fn get_map_renderer_proxy_for_journey_date_range(
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
) -> Result<MapRendererProxy> {
    let state = get();
    let journey_bitmap = state.storage.with_db_txn(|txn| {
        merged_journey_builder::get_range(txn, from_date_inclusive, to_date_inclusive, None)
    })?;

    let mut server = state.map_server.lock().unwrap();
    let map_renderer = MapRenderer::new(journey_bitmap);
    let token = server.register_map_renderer(Arc::new(Mutex::new(map_renderer)));
    Ok(MapRendererProxy::Token(token))
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
            tile.iter().next().map(|(block_key, _)| {
                let blockzoomed_x: i32 =
                    TILE_WIDTH as i32 * tile_pos.0 as i32 + block_key.x() as i32;
                let blockzoomed_y: i32 =
                    TILE_WIDTH as i32 * tile_pos.1 as i32 + block_key.y() as i32;
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

fn get_map_renderer_proxy_for_journey_data_internal(
    state: &'static MainState,
    journey_data: JourneyData,
) -> Result<(MapRendererProxy, Option<CameraOption>)> {
    let journey_bitmap = match journey_data {
        JourneyData::Bitmap(bitmap) => bitmap,
        JourneyData::Vector(vector) => {
            let mut bitmap = JourneyBitmap::new();
            merged_journey_builder::add_journey_vector_to_journey_bitmap(&mut bitmap, &vector);
            bitmap
        }
    };

    let default_camera_option = get_default_camera_option_from_journey_bitmap(&journey_bitmap);

    let map_renderer = MapRenderer::new(journey_bitmap);
    let mut server = state.map_server.lock().unwrap();
    let token = server.register_map_renderer(Arc::new(Mutex::new(map_renderer)));
    Ok((MapRendererProxy::Token(token), default_camera_option))
}

pub fn get_map_renderer_proxy_for_journey(
    journey_id: &str,
) -> Result<(MapRendererProxy, Option<CameraOption>)> {
    let state = get();
    let journey_data = state
        .storage
        .with_db_txn(|txn| txn.get_journey_data(journey_id))?;
    get_map_renderer_proxy_for_journey_data_internal(state, journey_data)
}

pub fn get_map_renderer_proxy_for_journey_data(
    journey_data: &JourneyData,
) -> Result<(MapRendererProxy, Option<CameraOption>)> {
    let state = get();
    // TODO: the clone here is not ideal, we should redesign the interface,
    // maybe consider Arc.
    get_map_renderer_proxy_for_journey_data_internal(state, journey_data.clone())
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
    let mut map_renderer = state.main_map_renderer.lock().unwrap();

    raw_data_list.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
    raw_data_list.into_iter().for_each(|raw_data| {
        // TODO: more batching updates
        let last_point = gps_preprocessor.last_kept_point();
        let process_result = gps_preprocessor.preprocess(&raw_data);
        let line_to_add = match process_result {
            ProcessResult::Ignore => None,
            ProcessResult::NewSegment => Some((&raw_data.point, &raw_data.point)),
            ProcessResult::Append => {
                let start = last_point.as_ref().unwrap_or(&raw_data.point);
                Some((start, &raw_data.point))
            }
        };
        match line_to_add {
            None => (),
            Some((start, end)) => {
                map_renderer.update(|journey_bitmap, tile_changed| {
                    journey_bitmap.add_line_with_change_callback(
                        start.longitude,
                        start.latitude,
                        end.longitude,
                        end.latitude,
                        tile_changed,
                    );
                });
            }
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

pub enum LayerKind {
    All,
    DefaultKind,
    Flight,
}

impl LayerKind {
    fn to_internal(&self) -> InternalLayerKind {
        match self {
            LayerKind::All => InternalLayerKind::All,
            LayerKind::DefaultKind => InternalLayerKind::JounreyKind(JourneyKind::DefaultKind),
            LayerKind::Flight => InternalLayerKind::JounreyKind(JourneyKind::Flight),
        }
    }

    fn of_internal(internal: &InternalLayerKind) -> LayerKind {
        match internal {
            InternalLayerKind::All => LayerKind::All,
            InternalLayerKind::JounreyKind(kind) => match kind {
                JourneyKind::DefaultKind => LayerKind::DefaultKind,
                JourneyKind::Flight => LayerKind::Flight,
            },
        }
    }
}

#[frb(sync)]
pub fn get_current_map_layer_kind() -> LayerKind {
    LayerKind::of_internal(&get().main_map_layer_kind.lock().unwrap())
}

pub fn set_main_map_layer_kind(layer_kind: LayerKind) -> Result<()> {
    let state = get();
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let mut main_map_layer_kind = state.main_map_layer_kind.lock().unwrap();

    let layer_kind = layer_kind.to_internal();
    let journey_bitmap = state
        .storage
        .get_latest_bitmap_for_main_map_renderer(&layer_kind)?;
    map_renderer.replace(journey_bitmap);
    *main_map_layer_kind = layer_kind;

    Ok(())
}

fn reset_gps_preprocessor_if_finalized<F>(finalize_op: F) -> Result<bool>
where
    F: FnOnce(&mut main_db::Txn) -> Result<bool>,
{
    let state = get();
    // TODO: I think we need to hold the gps_preprocessor lock first, otherwise
    // we might have a deadlock because the locking story in `on_location_update`
    // is quite complex. We should fix all the locking mess.
    let mut gps_preprocessor = state.gps_preprocessor.lock().unwrap();
    let finalized = state.storage.with_db_txn(finalize_op)?;
    // when journey is finalzied, we should reset the gps_preprocessor to prevent old state affecting new journey
    if finalized {
        *gps_preprocessor = GpsPreprocessor::new();
    }
    Ok(finalized)
}

pub fn finalize_ongoing_journey() -> Result<bool> {
    reset_gps_preprocessor_if_finalized(|txn| txn.finalize_ongoing_journey())
}

pub fn try_auto_finalize_journy() -> Result<bool> {
    reset_gps_preprocessor_if_finalized(|txn| txn.try_auto_finalize_journy())
}

pub fn has_ongoing_journey() -> Result<bool> {
    Ok(get()
        .storage
        .with_db_txn(|txn| txn.get_ongoing_journey_timestamp_range())?
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
        .with_db_txn(|txn| txn.get_journey_data(&journey_id))?;
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
            journeyinfo.journey_kind,
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

pub fn main_db_require_optimization() -> Result<bool> {
    get().storage.with_db_txn(|txn| txn.require_optimization())
}

pub fn optimize_main_db() -> Result<()> {
    get().storage.with_db_txn(|txn| txn.optimize())
}

pub fn area_of_main_map() -> u64 {
    let mut main_map_renderer = get().main_map_renderer.lock().unwrap();
    main_map_renderer.get_current_area()
}

pub fn restart_map_server() -> Result<()> {
    let state = get();
    let mut map_server = state.map_server.lock().unwrap();
    map_server.restart()
}

pub fn rebuild_cache() -> Result<()> {
    let state = get();
    state.storage.clear_all_cache()?;
    let bitmap = state
        .storage
        .get_latest_bitmap_for_main_map_renderer(&InternalLayerKind::All)?;
    state.main_map_renderer.lock().unwrap().replace(bitmap);
    Ok(())
}

// This is used for showing additional prompt to the user when trying to import
// multiple FoW data. Bitmap does not necessarily mean FoW data, but this is
// good enough.
pub fn contains_bitmap_journey() -> Result<bool> {
    // TODO: we should just have a real SQL query for this, instead of a liner
    // scan that involves deserializing all journey heads.
    let journey_headers = get()
        .storage
        .with_db_txn(|txn| txn.query_journeys(None, None))?;

    Ok(journey_headers
        .iter()
        .any(|header| match header.journey_type {
            JourneyType::Bitmap => true,
            JourneyType::Vector => false,
        }))
}

/// flutter_rust_bridge:ignore
pub mod for_testing {
    use std::sync::{Arc, Mutex};

    use crate::renderer::MapRenderer;

    pub fn get_main_map_renderer() -> Arc<Mutex<MapRenderer>> {
        super::get().main_map_renderer.clone()
    }
}

pub fn handle_webview_requests(request: String) -> Result<String> {
    let state = get();
    let registry = state.map_server.lock().unwrap().get_registry();
    let request = Request::parse(&request)?;
    let response = request.handle(registry);
    // Direct JSON serialization - more explicit and efficient
    serde_json::to_string(&response)
        .map_err(|e| anyhow::anyhow!("Failed to serialize response: {}", e))
}

#[frb(sync)]
pub fn get_mapbox_access_token() -> String {
    env!("MAPBOX-ACCESS-TOKEN").to_string()
}
