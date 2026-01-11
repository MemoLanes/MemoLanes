use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_with::{base64::Base64, serde_as};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use uuid::Uuid;

use super::MapRenderer;

use rand::RngCore;

// Registry now stores only a single MapRenderer (the last one set)
// The id in queries is kept for compatibility but the registry returns its only renderer
pub type Registry = Option<Arc<Mutex<MapRenderer>>>;

pub fn register_map_renderer(
    registry: Arc<Mutex<Registry>>,
    map_renderer: Arc<Mutex<MapRenderer>>,
) -> MapRendererToken {
    let id = Uuid::new_v4();
    {
        let mut registry = registry.lock().unwrap();
        // Replace the previous renderer with the new one
        *registry = Some(map_renderer);
    }
    MapRendererToken {
        id,
        registry: Arc::downgrade(&registry),
        is_primitive: true,
    }
}

pub struct MapRendererToken {
    pub id: Uuid,
    pub registry: Weak<Mutex<Registry>>,
    pub is_primitive: bool,
}

impl MapRendererToken {
    pub fn journey_id(&self) -> String {
        self.id.to_string()
    }

    pub fn get_map_renderer(&self) -> Option<Arc<Mutex<MapRenderer>>> {
        // Returns the single renderer regardless of the id
        if let Some(registry) = self.registry.upgrade() {
            let registry = registry.lock().unwrap();
            return registry.clone();
        }
        None
    }

    pub fn unregister(&self) {
        // Clear the single renderer from registry
        if let Some(registry) = self.registry.upgrade() {
            let mut registry = registry.lock().unwrap();
            *registry = None;
        }
    }

    // TODO: this is a workaround for returning main map without server-side lifetime control
    // we should remove this when we have a better way to handle the main map token
    pub fn clone_temporary_token(&self) -> MapRendererToken {
        MapRendererToken {
            id: self.id,
            registry: self.registry.clone(),
            is_primitive: false,
        }
    }
}

impl Drop for MapRendererToken {
    fn drop(&mut self) {
        if self.is_primitive {
            self.unregister();
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TileRangeQuery {
    pub x: i64,
    pub y: i64,
    pub z: i16,
    pub width: i64,
    pub height: i64,
    pub buffer_size_power: i16,
    pub cached_version: Option<String>,
}

#[serde_as]
#[derive(Deserialize, Serialize)]
pub struct TileRangeResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    #[serde_as(as = "Base64")]
    pub body: Vec<u8>,
}

// Random data generation query parameters
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RandomDataQuery {
    pub size: Option<u64>, // Size in bytes, default 1MB
}

// Unified request interface
#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "query", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum RequestPayload {
    TileRange(TileRangeQuery),
    RandomData(RandomDataQuery),
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub request_id: String,
    #[serde(flatten)]
    pub payload: RequestPayload,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestResponse<T> {
    pub request_id: String,
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

// Note: We can serialize this struct directly using serde_json::to_string()
// No need for Display trait implementation when we just want JSON

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end_random_data_request() {
        let request_json = r#"{
            "requestId": "test-123",
            "query": "random_data",
            "payload": {
                "size": 1024
            }
        }"#;

        let request = Request::parse(request_json).expect("Failed to parse request");

        // Create a dummy registry (None is fine for IPC test with random_data)
        let registry = Arc::new(Mutex::new(None));

        let response = request.handle(registry);

        // Verify response structure
        assert_eq!(response.request_id, "test-123");
        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());

        // Verify data structure
        let data = response.data.as_ref().unwrap();
        assert!(data["size"].as_u64().is_some());
        assert_eq!(data["size"].as_u64().unwrap(), 1024);
        assert!(data["data"].as_str().is_some()); // base64 encoded data

        // Verify JSON serialization works
        let json = serde_json::to_string(&response).expect("Failed to serialize response");
        assert!(json.contains("\"requestId\":\"test-123\""));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_invalid_request_parsing() {
        let invalid_json = r#"{
            "requestId": "test-456",
            "query": "unknown_type",
            "payload": {}
        }"#;

        let result = Request::parse(invalid_json);
        assert!(
            result.is_err(),
            "Expected parsing to fail for unknown query type"
        );
    }

    #[test]
    fn test_tile_range_request_includes_version() {
        use crate::journey_bitmap::JourneyBitmap;

        // Create a dummy journey bitmap
        let journey_bitmap = JourneyBitmap::new();
        let map_renderer = MapRenderer::new(journey_bitmap);

        // Create registry and set the single map renderer
        let registry = Arc::new(Mutex::new(Some(Arc::new(Mutex::new(map_renderer)))));

        // Create tile range request
        let request_json = r#"{
            "requestId": "test-version-123",
            "query": "tile_range",
            "payload": {
                "x": 0,
                "y": 0,
                "z": 0,
                "width": 1,
                "height": 1,
                "buffer_size_power": 6,
                "cached_version": null
            }
        }"#;

        let request = Request::parse(&request_json).expect("Failed to parse request");
        let response = request.handle(registry);

        // Verify response structure
        assert_eq!(response.request_id, "test-version-123");
        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());

        // Verify headers contain version
        let data = response.data.as_ref().unwrap();
        let headers = data["headers"].as_object().unwrap();
        assert!(
            headers.contains_key("version"),
            "Response headers should contain version"
        );

        // Verify version is a valid hex string
        let version = headers["version"].as_str().unwrap();
        assert!(!version.is_empty(), "Version should not be empty");
        // Version should be a hex string (can be parsed as hex)
        assert!(
            u64::from_str_radix(version, 16).is_ok(),
            "Version should be valid hex string"
        );
    }

    #[test]
    fn test_tile_range_request_returns_304_when_no_changes() {
        use crate::journey_bitmap::JourneyBitmap;

        // Create a dummy journey bitmap
        let journey_bitmap = JourneyBitmap::new();
        let map_renderer = MapRenderer::new(journey_bitmap);
        let version = map_renderer.get_version_string();

        // Create registry and set the single map renderer
        let registry = Arc::new(Mutex::new(Some(Arc::new(Mutex::new(map_renderer)))));

        // Create tile range query with current version (should trigger 302)
        let query = TileRangeQuery {
            x: 0,
            y: 0,
            z: 0,
            width: 1,
            height: 1,
            buffer_size_power: 6,
            cached_version: Some(version), // Use current version to trigger no-change scenario
        };

        let response = handle_tile_range_query(&query, registry);

        // Verify 304 response
        assert!(response.is_ok(), "Expected successful response");
        let tile_response = response.unwrap();
        assert_eq!(
            tile_response.status, 304,
            "Expected 304 status when no changes"
        );
        assert!(
            tile_response.headers.is_empty(),
            "Expected empty headers for 304 response"
        );
        assert!(
            tile_response.body.is_empty(),
            "Expected empty body for 304 response"
        );
    }
}

/// Handle TileRangeQuery with a MapRenderer reference directly (no registry lookup)
pub fn handle_tile_range_query_with_renderer(
    query: &TileRangeQuery,
    map_renderer: &MapRenderer,
) -> Result<TileRangeResponse, String> {
    // Get the latest bitmap if it has changed
    let (_, version) =
        match map_renderer.get_latest_bitmap_if_changed(query.cached_version.as_deref()) {
            None => {
                // No changes since client's cached version - return 304 status
                return Ok(TileRangeResponse {
                    status: 304,
                    headers: HashMap::new(),
                    body: Vec::new(),
                });
            }
            Some((journey_bitmap, version)) => (journey_bitmap, version),
        };

    // Generate tile buffer from journey bitmap
    let tile_buffer = match map_renderer.get_tile_buffer(
        query.x,
        query.y,
        query.z,
        query.width,
        query.height,
        query.buffer_size_power,
    ) {
        Ok(buffer) => buffer,
        Err(e) => return Err(format!("Failed to generate tile buffer: {e}")),
    };

    // Convert tile buffer to bytes and create response
    match tile_buffer.to_bytes() {
        Ok(data) => {
            let response_data = TileRangeResponse {
                status: 200,
                headers: {
                    let mut h = HashMap::new();
                    h.insert("version".to_string(), version);
                    h
                },
                body: data,
            };
            Ok(response_data)
        }
        Err(e) => Err(format!("Failed to serialize tile buffer: {e}")),
    }
}

/// Handle TileRangeQuery and return TileRangeResponse (looks up MapRenderer from registry)
/// Note: The id in query is ignored; the registry returns its only renderer
pub fn handle_tile_range_query(
    query: &TileRangeQuery,
    registry: Arc<Mutex<Registry>>,
) -> Result<TileRangeResponse, String> {
    // Get the single map renderer from registry (id is ignored)
    let locked_registry = registry.lock().unwrap();
    let map_renderer = match locked_registry.as_ref() {
        Some(item) => item.lock().unwrap(),
        None => return Err("Map renderer not found".to_string()),
    };

    handle_tile_range_query_with_renderer(query, &map_renderer)
}

impl Request {
    /// Parse a JSON string into a Request
    pub fn parse(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Handle the request and return an appropriate response
    pub fn handle(&self, registry: Arc<Mutex<Registry>>) -> RequestResponse<serde_json::Value> {
        match &self.payload {
            RequestPayload::TileRange(query) => match handle_tile_range_query(query, registry) {
                Ok(response_data) => match serde_json::to_value(response_data) {
                    Ok(value) => RequestResponse {
                        request_id: self.request_id.clone(),
                        success: true,
                        data: Some(value),
                        error: None,
                    },
                    Err(e) => RequestResponse {
                        request_id: self.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(format!("Failed to serialize response: {e}")),
                    },
                },
                Err(e) => RequestResponse {
                    request_id: self.request_id.clone(),
                    success: false,
                    data: None,
                    error: Some(e),
                },
            },
            RequestPayload::RandomData(query) => {
                let size = query.size.unwrap_or(1_048_576); // Default 1MB
                match generate_random_data(size) {
                    Ok(data) => {
                        let response_data = serde_json::json!({
                            "size": size,
                            "data": general_purpose::STANDARD.encode(&data)
                        });
                        RequestResponse {
                            request_id: self.request_id.clone(),
                            success: true,
                            data: Some(response_data),
                            error: None,
                        }
                    }
                    Err(e) => RequestResponse {
                        request_id: self.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(e),
                    },
                }
            }
        }
    }

    /// Handle the request with a MapRenderer reference directly (id in query is ignored)
    pub fn handle_map_renderer(
        &self,
        map_renderer: &MapRenderer,
    ) -> RequestResponse<serde_json::Value> {
        match &self.payload {
            RequestPayload::TileRange(query) => {
                match handle_tile_range_query_with_renderer(query, map_renderer) {
                    Ok(response_data) => match serde_json::to_value(response_data) {
                        Ok(value) => RequestResponse {
                            request_id: self.request_id.clone(),
                            success: true,
                            data: Some(value),
                            error: None,
                        },
                        Err(e) => RequestResponse {
                            request_id: self.request_id.clone(),
                            success: false,
                            data: None,
                            error: Some(format!("Failed to serialize response: {e}")),
                        },
                    },
                    Err(e) => RequestResponse {
                        request_id: self.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(e),
                    },
                }
            }
            RequestPayload::RandomData(query) => {
                let size = query.size.unwrap_or(1_048_576); // Default 1MB
                match generate_random_data(size) {
                    Ok(data) => {
                        let response_data = serde_json::json!({
                            "size": size,
                            "data": general_purpose::STANDARD.encode(&data)
                        });
                        RequestResponse {
                            request_id: self.request_id.clone(),
                            success: true,
                            data: Some(response_data),
                            error: None,
                        }
                    }
                    Err(e) => RequestResponse {
                        request_id: self.request_id.clone(),
                        success: false,
                        data: None,
                        error: Some(e),
                    },
                }
            }
        }
    }
}

// Move the random data generation to a separate function
pub fn generate_random_data(size: u64) -> Result<Vec<u8>, String> {
    let max_size = 10_485_760; // 10MB limit

    if size > max_size {
        return Err(format!(
            "Size too large. Maximum allowed: {max_size} bytes (10MB)"
        ));
    }

    // Generate random data efficiently
    let mut data = vec![0u8; size as usize];
    rand::rng().fill_bytes(&mut data);

    Ok(data)
}
