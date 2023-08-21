use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

use flutter_rust_bridge::ZeroCopyBuffer;
use simplelog::{Config, LevelFilter, WriteLogger};
use tiny_skia::{Pixmap, Color};

use crate::storage::Storage;

struct MainState {
    storage: Storage,
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
        MainState { storage }
    });
    if already_initialized {
        warn!("`init` is called multiple times");
    }
}

fn get() -> &'static MainState {
    MAIN_STATE.get().expect("main state is not initialized")
}

pub fn render_map_overlay() -> ZeroCopyBuffer<Vec<u8>> {
    // TODO: Change render backend. Right now we are using `tiny-skia`,
    // it should work just fine and we don't really need fancy features.
    // However, it is mostly a research project and does not feel like production ready,
    // `rust-skia` looks a lot better and has better performance (unlike `tiny-skia` is
    // purely on CPU, `rust-skia` can be ran on GPU). The reason we use `tiny-skia` right
    // now is that it is pure rust, so we don't need to think about how to build depenceies
    // for various platform.

    // TODO: reuse resources?
    let mut pixmap = Pixmap::new(128, 128).unwrap();
    pixmap.fill(Color::from_rgba8(0, 0, 0, 128));
    let bytes = pixmap.encode_png().unwrap();
    ZeroCopyBuffer(bytes)
}
