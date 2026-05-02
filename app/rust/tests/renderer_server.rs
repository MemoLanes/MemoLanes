pub mod test_utils;

// Include the shared examples module
#[path = "../examples/shared/mod.rs"]
mod examples_shared;
use examples_shared::MapServer;

use memolanes_core::import_data;
use memolanes_core::renderer::internal_server::Request;
use memolanes_core::renderer::MapRenderer;
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
        MapServer::create_and_start("localhost", None, map_renderer_fow)
            .expect("Failed to start server"),
    ));

    std::thread::sleep(Duration::from_millis(200));

    let request_str = r#"
    {
        "requestId": "test-123",
        "query": "tile_range",
        "payload": {
            "x": 0,
            "y": 0,
            "z": 0,
            "width": 1,
            "height": 1,
            "buffer_size_power": 6,
            "cached_version": "test-123"
        }
    }
    "#;

    // println!("request: {}", request_str);

    let request = Request::parse(request_str)?;

    // Get the map renderer and handle the request
    let mut map_renderer = map_renderer_clone.lock().unwrap();
    let response = request.handle(&mut map_renderer);
    drop(map_renderer);

    let body = response.data.as_ref().unwrap()["body"].as_str().unwrap();

    let body_for_compare = "AAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAABAAAAAAAAAAYAAQAAAAAAAADVAAAAAAAAACAAFQAgABYAIAAVACAAFQAhABQAIQAVACEAFAAhABQAIQAVACEAFAAhABQAIQAUACEAFQAhABUAIQAVACEAFAAiAB8AIgAUACIAFQAiABQAIgAVACIAFQAiABUAIgAVACIAFQAiABUAIgAVACIAFQAiABUAIgAVACIAFQAiABUAIgAVACIAFgAjABYAIwAWACMAFgAjABYAIwAWACMAFgAjABYAIwAWACMAFgAjABYAIwAXACMAFwAjABcAIwAXACMAFwAjABYAIwAWACMAFgAjABYAIwAWACMAFwAjABcAIwAXACMAFwAjABcAIwAXACMAFwAjABcAIwAXACMAFwAjABcAIwAXACMAFwAjABcAIwAYACMAFwAjABcAIwAYACMAGAAjABgAJAAXACQAGAAkABgAJAAYACQAGAAkABgAJAAYACQAGAAlABkAJgAZACkAGwAyABoAMwAaADMAGgAzABoAMwAaADMAGgAzABoAMwAaADMAGgAzABoAMwAaADMAGgAzABoAMwAaADMAGgAzABoAMwAaADQAGgA0ABoANAAaADQAGwA0ABsANAAaADQAGgA0ABoANAAbADQAGwA0ABsANAAaADQAGgA0ABsANAAbADQAGwA0ABoANAAaADQAGgA0ABsANAAbADQAGwA0ABsANAAbADQAGwA0ABsANAAbADQAGgA0ABoANAAaADQAGgA0ABoANAAaADQAGgA0ABsANAAbADQAGwA0ABsANAAbADQAGwA0ABsANAAaADQAGgA0ABoANAAaADQAGgA0ABsANAAbADQAGwA0ABsANAAbADQAGwA0ABsANAAaADQAGgA0ABoANAAaADQAGgA0ABsANAAbADQAGwA0ABsANAAaADQAGgA0ABoANAAaADQAGgA0ABsANAAbADQAGwA0ABsANQAZADUAGgA1ABoANQAaADUAGgA1ABoANQAaADUAGwA1ABsANQAbADUAGwA1ABoANQAaADUAGgA1ABoANQAaADUAGgA1ABsANQAbADUAGwA1ABsANQAaADUAGgA1ABoANQAaADUAGgA1ABoANQAaADUAGgA1ABsANQAbADUAGgA1ABoANQAaADUAGgA1ABoANQAaADUAGgA1ABsAOAAZAA==";

    assert_eq!(body, body_for_compare);

    // Direct JSON serialization - more explicit and efficient
    let response_str = serde_json::to_string(&response)
        .map_err(|e| anyhow::anyhow!("Failed to serialize response: {e}"))
        .unwrap();
    println!("response: {response_str}");

    Ok(())
}
