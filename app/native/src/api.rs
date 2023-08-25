use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

use simplelog::{Config, LevelFilter, WriteLogger};

use crate::map_renderer::{MapRenderer, RenderResult};
use crate::storage::Storage;

struct MainState {
    storage: Storage,
    map_renderer: MapRenderer,
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
            map_renderer: MapRenderer::new(),
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
