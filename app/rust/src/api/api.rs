use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use auto_context::auto_context;
use chrono::NaiveDate;
use csv::Reader;
use flutter_rust_bridge::frb;

use super::import::JourneyInfo;
use crate::cache_db::LayerKind;
use crate::frb_generated::StreamSink;
use crate::gps_processor::{GpsPreprocessor, ProcessResult};
use crate::journey_bitmap::JourneyBitmap;
use crate::journey_data::JourneyData;
use crate::journey_header::{JourneyHeader, JourneyKind, JourneyType};
use crate::logs;
use crate::renderer::get_default_camera_option_from_journey_bitmap;
use crate::renderer::internal_server::MapRendererToken;
use crate::renderer::internal_server::{register_map_renderer, Registry, Request};
use crate::renderer::MapRenderer;
use crate::storage::{RawDataFile, Storage};
use crate::{
    archive, build_info, export_data, gps_processor, main_db, merged_journey_builder, storage,
};

use crate::renderer::CameraOptionInternal;

type CameraOption = CameraOptionInternal;

use crate::export_data::raw_data_csv_to_gpx_file;
use log::{error, info, warn};

// TODO: we have way too many locking here and now it is hard to track.
//  e.g. we could mess up with the order and cause a deadlock
#[frb(ignore)]
pub(super) struct MainState {
    pub storage: Storage,
    pub gps_preprocessor: Mutex<GpsPreprocessor>,
    pub registry: Arc<Mutex<Registry>>,
    // TODO: we should reconsider the way we handle the main map
    main_map_state: Arc<Mutex<MainMapState>>,
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

#[auto_context]
fn reload_main_map_bitmap(
    storage: &Storage,
    main_map_renderer: &mut MapRenderer,
    main_map_state: &MainMapState,
) -> Result<()> {
    if main_map_state.dropped_for_power_saving {
        return Ok(());
    }

    let layer_filter = main_map_state.layer_filter;

    // TODO: merge layer filter with layer kind
    let layer_kind = match (layer_filter.default_kind, layer_filter.flight_kind) {
        (true, true) => Some(LayerKind::All),
        (true, false) => Some(LayerKind::JourneyKind(JourneyKind::DefaultKind)),
        (false, true) => Some(LayerKind::JourneyKind(JourneyKind::Flight)),
        (false, false) => None,
    };

    let journey_bitmap = storage
        .get_latest_bitmap_for_main_map_renderer(&layer_kind, layer_filter.current_journey)?;
    main_map_renderer.replace(journey_bitmap);
    Ok(())
}

pub fn init(temp_dir: String, doc_dir: String, support_dir: String, system_cache_dir: String) {
    let mut already_initialized = true;
    MAIN_STATE.get_or_init(|| {
        already_initialized = false;

        let (real_cache_dir, logs) = prepare_real_cache_dir(&support_dir, &system_cache_dir)
            .expect("Failed to initialize cache dir");

        // init logging
        logs::init(&real_cache_dir).expect("Failed to initialize logging");

        if let Some(logs) = logs {
            for (level, message) in logs {
                write_log(message, level);
            }
        }

        let mut storage = Storage::init(temp_dir, doc_dir, support_dir, real_cache_dir);
        info!("initialized");

        let registry = Arc::new(Mutex::new(Registry::new()));

        let default_layer_filter = LayerFilter {
            current_journey: true,
            default_kind: true,
            flight_kind: false,
        };

        let main_map_state = Arc::new(Mutex::new(MainMapState {
            dropped_for_power_saving: false,
            layer_filter: default_layer_filter,
        }));
        let main_map_state_copy = main_map_state.clone();
        // TODO: use an empty journey bitmap first, because loading could be slow (especially when we don't have cache).
        // Ideally, we should support main map renderer being none. e.g. we free it when the user is not using the map.
        let main_map_renderer = Arc::new(Mutex::new(MapRenderer::new(JourneyBitmap::new())));
        let main_map_renderer_copy = main_map_renderer.clone();
        // TODO: redesign the callback to better handle locks and avoid deadlocks
        storage.set_finalized_journey_changed_callback(Box::new(move |storage| {
            let mut map_renderer = main_map_renderer_copy.lock().unwrap();
            let main_map_state = main_map_state_copy.lock().unwrap();
            match reload_main_map_bitmap(storage, &mut map_renderer, &main_map_state) {
                Ok(()) => (),
                Err(e) => {
                    error!("Failed to get latest bitmap for main map renderer: {e:?}");
                }
            }
        }));
        let main_map_renderer_token =
            register_map_renderer(registry.clone(), main_map_renderer.clone());
        info!("main map renderer initialized");

        MainState {
            storage,
            gps_preprocessor: Mutex::new(GpsPreprocessor::new()),
            registry,
            main_map_state,
            main_map_renderer,
            main_map_renderer_token,
        }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

// On iOS, we use `NSCachesDirectory` for storing cache file,
// it won't be cleared by the system and also won't be included in icloud backup,
// which is exactly what we want.
// On Android, we don't use `getCacheDir()` but create our own folder under `getFilesDir()`.
// The reason is that on Android,
// the cache folder may be cleared even when the app is running,
// which is troublesome for us. Also the app request the whole cache while running,
// it will create the whole thing if missing so clearing the cache randomly doesn't provide much value.
#[allow(clippy::type_complexity)]
fn prepare_real_cache_dir(
    support_dir: &str,
    system_cache_dir: &str,
) -> Result<(String, Option<Vec<(LogLevel, String)>>)> {
    if std::env::consts::OS == "android" {
        let final_path = Path::new(support_dir).join("cache");
        // Migrate cache data
        let logs = if !final_path.exists() {
            let mut logs = Vec::new();
            logs.push((
                LogLevel::Info,
                format!("Setting up real cache dir for Android at {final_path:?}"),
            ));
            // TODO this can be delete when most people have rolled pass this.
            let old_dir = Path::new(system_cache_dir);
            if old_dir.exists() {
                logs.push((
                    LogLevel::Info,
                    format!("Old cache dir {old_dir:?} exists, move Data"),
                ));

                std::fs::create_dir_all(&final_path).map_err(|e| {
                    logs.push((
                        LogLevel::Error,
                        format!("Failed to create final cache dir {final_path:?}: {e:?}"),
                    ));
                    e
                })?;

                let old_db = old_dir.join("cache.db");
                let new_db = final_path.join("cache.db");

                if old_db.exists() {
                    logs.push((
                        LogLevel::Info,
                        format!("Found {old_db:?}, move to {new_db:?}"),
                    ));

                    match std::fs::rename(&old_db, &new_db) {
                        Ok(()) => logs.push((
                            LogLevel::Info,
                            format!("Successfully moved cache.db to {new_db:?}"),
                        )),
                        Err(e) => {
                            logs.push((LogLevel::Error, format!("Failed to move cache.db: {e:?}")))
                        }
                    }
                }

                let old_log = old_dir.join("logs");
                let new_log = final_path.join("logs");

                if old_log.exists() {
                    logs.push((
                        LogLevel::Info,
                        format!("Found log directory {old_log:?}, move to {new_log:?}"),
                    ));

                    match std::fs::rename(&old_log, &new_log) {
                        Ok(()) => logs.push((
                            LogLevel::Info,
                            format!("Successfully moved log directory to {new_log:?}"),
                        )),
                        Err(e) => logs.push((
                            LogLevel::Error,
                            format!("Failed to move log directory: {e:?}"),
                        )),
                    }
                }
            } else {
                logs.push((
                    LogLevel::Info,
                    format!("Old cache dir {old_dir:?} does not exist, no migration needed"),
                ));
                std::fs::create_dir_all(&final_path)?;
            }
            Some(logs)
        } else {
            None
        };
        Ok((final_path.to_string_lossy().into_owned(), logs))
    } else {
        Ok((system_cache_dir.to_string(), None))
    }
}

// TODO: this design is not ideal, we need this because the `init` above uses an empty one.
pub fn init_main_map() -> Result<()> {
    let state = get();
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let main_map_state = state.main_map_state.lock().unwrap();
    reload_main_map_bitmap(&state.storage, &mut map_renderer, &main_map_state)
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

    let map_renderer = MapRenderer::new(journey_bitmap);
    let token = register_map_renderer(state.registry.clone(), Arc::new(Mutex::new(map_renderer)));
    MapRendererProxy::Token(token)
}

pub fn get_map_renderer_proxy_for_journey_date_range(
    from_date_inclusive: NaiveDate,
    to_date_inclusive: NaiveDate,
) -> Result<MapRendererProxy> {
    let state = get();
    let journey_bitmap = state.storage.with_db_txn(|txn| {
        merged_journey_builder::get_range(txn, from_date_inclusive, to_date_inclusive, None)
    })?;

    let map_renderer = MapRenderer::new(journey_bitmap);
    let token = register_map_renderer(state.registry.clone(), Arc::new(Mutex::new(map_renderer)));

    Ok(MapRendererProxy::Token(token))
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
    let token = register_map_renderer(state.registry.clone(), Arc::new(Mutex::new(map_renderer)));
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

// Return `true` if this update contains meaningful data.
// Meaningful data means it is not ignored by the gps preprocessor.
pub fn on_location_update(raw_data: gps_processor::RawData, received_timestamp_ms: i64) -> bool {
    let state = get();
    // NOTE: On Android, we might received a batch of location updates that are out of order.
    // Not very sure why yet.

    // we need handle a batch in one go so we hold the lock for the whole time
    let mut gps_preprocessor = state.gps_preprocessor.lock().unwrap();
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let main_map_state = state.main_map_state.lock().unwrap();

    let last_point = gps_preprocessor.last_kept_point();
    let process_result = gps_preprocessor.preprocess(&raw_data);
    if !main_map_state.dropped_for_power_saving && main_map_state.layer_filter.current_journey {
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
        };
    };

    state
        .storage
        .record_gps_data(&raw_data, process_result, received_timestamp_ms);

    match process_result {
        ProcessResult::Ignore => false,
        ProcessResult::Append | ProcessResult::NewSegment => true,
    }
}

pub fn list_all_raw_data() -> Result<Vec<RawDataFile>> {
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

#[frb]
#[derive(Eq, Clone, Copy, Debug, PartialEq)]
pub struct LayerFilter {
    #[frb(non_final)]
    pub current_journey: bool,
    #[frb(non_final)]
    pub default_kind: bool,
    #[frb(non_final)]
    pub flight_kind: bool,
}

struct MainMapState {
    dropped_for_power_saving: bool,
    layer_filter: LayerFilter,
}

#[frb(sync)]
pub fn get_current_main_map_layer_filter() -> LayerFilter {
    get().main_map_state.lock().unwrap().layer_filter
}

pub fn set_main_map_layer_filter(new_layer_filter: &LayerFilter) -> Result<()> {
    let state = get();
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let mut main_map_state = state.main_map_state.lock().unwrap();

    if *new_layer_filter != main_map_state.layer_filter {
        main_map_state.layer_filter = *new_layer_filter;
        reload_main_map_bitmap(&state.storage, &mut map_renderer, &main_map_state)?;
    }
    Ok(())
}

#[auto_context]
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
    // when journey is finalized, we should reset the gps_preprocessor to prevent old state affecting new journey
    if finalized {
        *gps_preprocessor = GpsPreprocessor::new();
    }
    Ok(finalized)
}

pub fn finalize_ongoing_journey() -> Result<bool> {
    reset_gps_preprocessor_if_finalized(|txn| txn.finalize_ongoing_journey())
}

pub fn try_auto_finalize_journey() -> Result<bool> {
    reset_gps_preprocessor_if_finalized(|txn| txn.try_auto_finalize_journey())
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

pub fn list_journey_on_date(year: i32, month: u32, day: u32) -> Result<Vec<JourneyHeader>> {
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

#[auto_context]
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

#[auto_context]
pub fn export_raw_data_gpx_file(csv_filepath: String) -> Result<String> {
    let csv_path = Path::new(&csv_filepath);
    let file_name = csv_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse filename: {}", csv_filepath))?;

    let target_dir = Path::new(&get().storage.cache_dir).join("raw_data");

    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    let gpx_path = target_dir.join(file_name).with_extension("gpx");
    let gpx_path_str = gpx_path.to_string_lossy().to_string();

    if gpx_path.exists() {
        return Ok(gpx_path_str);
    }

    let csv_file = File::open(csv_path)
        .with_context(|| format!("Failed to open source CSV file: {}", csv_filepath))?;
    let mut reader = Reader::from_reader(BufReader::new(csv_file));

    let gpx_file = File::create(&gpx_path)
        .with_context(|| format!("Failed to create target GPX file: {}", gpx_path_str))?;

    let mut writer = BufWriter::new(gpx_file);

    raw_data_csv_to_gpx_file(&mut reader, &mut writer)
        .with_context(|| format!("Failed to convert CSV to GPX: {}", csv_filepath))?;

    Ok(gpx_path_str)
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

pub fn update_journey_metadata(id: &str, journey_info: JourneyInfo) -> Result<()> {
    get().storage.with_db_txn(|txn| {
        txn.update_journey_metadata(
            id,
            journey_info.journey_date,
            journey_info.start_time,
            journey_info.end_time,
            journey_info.note,
            journey_info.journey_kind,
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

pub fn area_of_main_map() -> Option<u64> {
    let state = get();
    let mut main_map_renderer = state.main_map_renderer.lock().unwrap();
    let main_map_state = state.main_map_state.lock().unwrap();
    if main_map_state.dropped_for_power_saving {
        None
    } else {
        Some(main_map_renderer.get_current_area())
    }
}

pub fn rebuild_cache() -> Result<()> {
    let state = get();
    state.storage.clear_all_cache()?;
    let mut map_renderer = state.main_map_renderer.lock().unwrap();
    let main_map_state = state.main_map_state.lock().unwrap();
    reload_main_map_bitmap(&state.storage, &mut map_renderer, &main_map_state)
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
    let registry = state.registry.clone();
    let request = Request::parse(&request)?;
    let response = request.handle(registry);
    // Direct JSON serialization - more explicit and efficient
    serde_json::to_string(&response)
        .map_err(|e| anyhow::anyhow!("Failed to serialize response: {e}"))
}

#[frb(ignore)]
pub fn get_registry() -> Arc<Mutex<Registry>> {
    get().registry.clone()
}

#[frb(sync)]
pub fn get_mapbox_access_token() -> Option<String> {
    build_info::MAPBOX_ACCESS_TOKEN.map(|x| x.to_string())
}

pub fn free_resource_for_long_time_background() {
    let state = get();
    let mut main_map_renderer = state.main_map_renderer.lock().unwrap();
    let mut main_map_state = state.main_map_state.lock().unwrap();
    // TODO: ideally we want to free the whole map renderer and make it optional.
    // The current approach of having a flag and replacing the bitmap with an
    // empty one is a bit error-prone.
    if !main_map_state.dropped_for_power_saving {
        main_map_state.dropped_for_power_saving = true;
        main_map_renderer.replace(JourneyBitmap::new());
        info!("Journey bitmap for the main map is dropped for power saving.");
    }
}

pub fn reload_resource_for_foreground() -> Result<()> {
    let state = get();
    let mut main_map_renderer = state.main_map_renderer.lock().unwrap();
    let mut main_map_state = state.main_map_state.lock().unwrap();
    if main_map_state.dropped_for_power_saving {
        info!("loading back main map");
        main_map_state.dropped_for_power_saving = false;
        reload_main_map_bitmap(&state.storage, &mut main_map_renderer, &main_map_state)?;
        info!("main map loaded");
    }
    Ok(())
}
