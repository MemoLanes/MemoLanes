pub mod test_utils;

use memolanes_core::import_data;
use memolanes_core::renderer::internal_server::dispatch_request;
use memolanes_core::renderer::MapRenderer;
#[path = "../examples/shared/mod.rs"]
mod shared;
use shared::MapServer;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[test]
pub fn renderer_server() -> Result<(), Box<dyn std::error::Error>> {
    let (joruney_bitmap_fow, _) =
        import_data::load_fow_sync_data("./tests/data/fow_3.zip").unwrap();
    let map_renderer_fow = Arc::new(Mutex::new(MapRenderer::new(joruney_bitmap_fow)));
    let map_renderer_clone = map_renderer_fow.clone();

    let _server = Arc::new(Mutex::new(
        MapServer::create_and_start(map_renderer_fow).expect("Failed to start server"),
    ));

    std::thread::sleep(Duration::from_millis(200));

    let params: HashMap<String, String> = [
        ("x", "0"),
        ("y", "0"),
        ("z", "0"),
        ("width", "1"),
        ("height", "1"),
        ("buffer_size_power", "6"),
        ("cached_version", "test-123"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    let mut map_renderer = map_renderer_clone.lock().unwrap();
    let response = dispatch_request("tile_range", &params, &mut map_renderer);
    drop(map_renderer);

    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/octet-stream");

    // The response should contain tile data (non-empty body with a version header)
    assert!(response.headers.contains_key("X-Tile-Version"));
    assert!(!response.body.is_empty());

    println!(
        "response: status={}, body_len={}, version={:?}",
        response.status,
        response.body.len(),
        response.headers.get("X-Tile-Version")
    );

    Ok(())
}
