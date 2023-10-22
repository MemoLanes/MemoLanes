use std::fs::File;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use simplelog::{Config, LevelFilter, WriteLogger};

use crate::gps_processor::GpsProcessor;
use crate::journey_bitmap::JourneyBitmap;
use crate::map_renderer::{MapRenderer, RenderResult};
use crate::storage::Storage;
use crate::{gps_processor, storage};

struct MainState {
    storage: Storage,
    map_renderer: Mutex<MapRenderer>,
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
            map_renderer: Mutex::new(MapRenderer::new(JourneyBitmap::new())),
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
    let mut map_renderer = get().map_renderer.lock().unwrap();
    map_renderer.maybe_render_map_overlay(zoom, left, top, right, bottom)
}

pub fn on_location_update(
    latitude: f64,
    longitude: f64,
    timestamp_ms: i64,
    accuracy: f32,
    altitude: Option<f32>,
    speed: Option<f32>,
) {
    let state = get();
    let raw_data = gps_processor::RawData {
        latitude,
        longitude,
        timestamp_ms,
        accuracy,
        altitude,
        speed,
    };
    let process_result = state.gps_processor.lock().unwrap().process(&raw_data);
    state.storage.record_gps_data(&raw_data, process_result);
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

pub fn finalize_ongoing_journey() {
    get().storage.finalize_ongoing_journey()
}
