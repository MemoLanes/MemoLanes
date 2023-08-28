use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

use simplelog::{Config, LevelFilter, WriteLogger};

use crate::gps_processor::GpsProcessor;
use crate::map_renderer::{MapRenderer, RenderResult};
use crate::storage::Storage;
use crate::{gps_processor, storage};

struct MainState {
    storage: Storage,
    map_renderer: MapRenderer,
    gps_processor: GpsProcessor,
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

        // TODO: make this `None` by default
        storage.toggle_raw_data_mode(true);

        MainState {
            storage,
            map_renderer: MapRenderer::new(),
            gps_processor: GpsProcessor::new(),
        }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

pub fn render_map_overlay(zoom: f32, left: f64, top: f64, right: f64, bottom: f64) -> RenderResult {
    get()
        .map_renderer
        .render_map_overlay(zoom, left, top, right, bottom)
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
    let process_result = state.gps_processor.process(&raw_data);
    state.storage.record_gps_data(&raw_data, process_result);
}

pub fn list_all_raw_data() -> Vec<storage::RawDataFile> {
    get().storage.list_all_raw_data()
}
