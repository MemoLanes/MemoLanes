use actix_web::{
    dev::ServerHandle, http::Method, web, App, HttpResponse, HttpResponseBuilder, HttpServer,
};
use anyhow::Result;
use std::borrow::Cow;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::generate_random_data;
use super::internal_server::{
    handle_tile_range_query, RandomDataQuery, Registry, Request, TileRangeQuery,
};
use super::MapRenderer;

pub struct MapRendererToken {
    id: Uuid,
    registry: Weak<Mutex<Registry>>,
    server_info: Arc<Mutex<ServerInfo>>,
    is_primitive: bool,
}

impl MapRendererToken {
    pub fn url(&self) -> String {
        let server_info = self.server_info.lock().unwrap();
        format!(
            "http://{}:{}/#journey_id={}",
            server_info.host, server_info.port, self.id
        )
    }

    pub fn url_hash_params(&self) -> String {
        let server_info = self.server_info.lock().unwrap();
        format!(
            "#cgi_endpoint=http%3A%2F%2F{}%3A{}&journey_id={}&access_key={}",
            server_info.host,
            server_info.port,
            self.id,
            env!("MAPBOX-ACCESS-TOKEN")
        )
    }

    pub fn journey_id(&self) -> String {
        self.id.to_string()
    }

    pub fn get_map_renderer(&self) -> Option<Arc<Mutex<MapRenderer>>> {
        if let Some(registry) = self.registry.upgrade() {
            let registry = registry.lock().unwrap();
            return registry.get(&self.id).cloned();
        }
        None
    }

    pub fn unregister(&self) {
        if let Some(registry) = self.registry.upgrade() {
            let mut registry = registry.lock().unwrap();
            registry.remove(&self.id);
        }
    }

    // TODO: this is a workaround for returning main map without server-side lifetime control
    // we should remove this when we have a better way to handle the main map token
    pub fn clone_temporary_token(&self) -> MapRendererToken {
        MapRendererToken {
            id: self.id,
            registry: self.registry.clone(),
            server_info: self.server_info.clone(),
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

/// Helper function to add standard CORS headers to an HttpResponseBuilder
fn add_cors_headers(builder: &mut HttpResponseBuilder) -> &mut HttpResponseBuilder {
    builder
        .append_header(("Access-Control-Allow-Origin", "*"))
        .append_header(("Access-Control-Allow-Methods", "GET, OPTIONS"))
        .append_header((
            "Access-Control-Allow-Headers",
            "Content-Type, If-None-Match",
        ))
}

async fn serve_journey_tile_range(
    query: web::Query<TileRangeQuery>,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    // Use the tile-specific handler directly
    match handle_tile_range_query(query.into_inner(), data.get_ref().clone()) {
        Ok(tile_response) => {
            match tile_response.status {
                200 => {
                    // Success - return the tile data
                    add_cors_headers(&mut HttpResponse::Ok())
                        .content_type("application/json")
                        .body(
                            serde_json::to_string(&tile_response)
                                .unwrap_or_else(|_| "{}".to_string()),
                        )
                }
                304 => {
                    // Not Modified - client's cached version is still valid
                    add_cors_headers(&mut HttpResponse::NotModified()).finish()
                }
                _ => {
                    // Other status codes - treat as server error
                    add_cors_headers(&mut HttpResponse::InternalServerError())
                        .content_type("text/plain")
                        .body(format!("Unexpected status: {}", tile_response.status))
                }
            }
        }
        Err(error_message) => {
            if error_message.contains("Map renderer not found") {
                add_cors_headers(&mut HttpResponse::NotFound())
                    .content_type("text/plain")
                    .body(error_message)
            } else {
                add_cors_headers(&mut HttpResponse::InternalServerError())
                    .content_type("text/plain")
                    .body(error_message)
            }
        }
    }
}

// Unified JSON POST endpoint that handles all request types
async fn serve_unified_json_request(
    body: web::Bytes,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    // Parse the JSON request
    let request_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return add_cors_headers(&mut HttpResponse::BadRequest())
                .content_type("application/json")
                .body(r#"{"error": "Invalid UTF-8 in request body"}"#);
        }
    };

    let request = match Request::parse(request_str) {
        Ok(req) => req,
        Err(e) => {
            return add_cors_headers(&mut HttpResponse::BadRequest())
                .content_type("application/json")
                .body(format!(r#"{{"error": "Failed to parse request: {}"}}"#, e));
        }
    };

    // Handle the request using the unified interface
    let response = request.handle(data.get_ref().clone());

    // Convert to JSON and return HTTP response
    match serde_json::to_string(&response) {
        Ok(json) => {
            let status_code = if response.success { 200 } else { 400 };
            add_cors_headers(&mut HttpResponse::build(
                actix_web::http::StatusCode::from_u16(status_code)
                    .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
            ))
            .content_type("application/json")
            .body(json)
        }
        Err(e) => add_cors_headers(&mut HttpResponse::InternalServerError())
            .content_type("application/json")
            .body(format!(
                r#"{{"error": "Failed to serialize response: {}"}}"#,
                e
            )),
    }
}

// Simplified HTTP handler - now uses the shared function
async fn serve_ipc_test_download(query: web::Query<RandomDataQuery>) -> HttpResponse {
    let size = query.size.unwrap_or(1_048_576); // Default 1MB

    match generate_random_data(size) {
        Ok(data) => add_cors_headers(&mut HttpResponse::Ok())
            .append_header(("Content-Length", size.to_string()))
            .content_type("application/octet-stream")
            .body(data),
        Err(error_msg) => add_cors_headers(&mut HttpResponse::BadRequest())
            .content_type("text/plain")
            .body(error_msg),
    }
}

pub struct ServerInfo {
    host: String,
    port: u16,
}

pub struct MapServer {
    handles: Option<(JoinHandle<()>, ServerHandle)>,
    registry: Arc<Mutex<Registry>>,
    server_info: Arc<Mutex<ServerInfo>>,
}

impl MapServer {
    fn start_server_blocking<F>(
        host: &str,
        port: Option<u16>,
        registry: Arc<Mutex<Registry>>,
        ready_with_port: F,
    ) -> Result<()>
    where
        F: FnOnce(u16, ServerHandle),
    {
        let port = match port {
            Some(0) => None,
            x => x,
        };
        let runtime = Runtime::new()?;
        runtime.block_on(async move {
            info!("Setting up map server ...");
            let data = web::Data::new(registry);
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(data.clone())
                    .route("/tile_range", web::get().to(serve_journey_tile_range))
                    .route(
                        "/tile_range",
                        web::method(Method::OPTIONS).to(handle_preflight),
                    )
                    // Unified JSON POST endpoint
                    .route("/api", web::post().to(serve_unified_json_request))
                    .route("/api", web::method(Method::OPTIONS).to(handle_preflight))
                    // Only one HTTP test endpoint
                    .route("/random_data", web::get().to(serve_ipc_test_download))
                    .route(
                        "/random_data",
                        web::method(Method::OPTIONS).to(handle_preflight),
                    )
                    .route("/", web::get().to(index))
                    .route("/main.bundle.js", web::get().to(serve_journey_kernel_js))
                    .route(
                        "/journey_kernel_bg.wasm",
                        web::get().to(serve_journey_kernel_wasm),
                    )
            })
            .bind((host, port.unwrap_or(0)))?
            .workers(1)
            .shutdown_timeout(0);

            let actual_port = match port {
                Some(port) => port,
                None => {
                    if let Some(addr) = server.addrs().first() {
                        addr.port()
                    } else {
                        return Err(anyhow!("Failed to get server address"));
                    }
                }
            };
            info!("Server bound successfully to {host}:{actual_port}");

            let server = server.run();
            let server_handle = server.handle();

            ready_with_port(actual_port, server_handle);

            server.await?;
            Ok(())
        })
    }

    fn start_server(
        host: &str,
        port: Option<u16>,
        registry: Arc<Mutex<Registry>>,
    ) -> Result<((JoinHandle<()>, ServerHandle), ServerInfo)> {
        let (tx, rx) = std::sync::mpsc::channel();
        let host_for_move = host.to_owned();
        let handle = thread::spawn(move || {
            if let Err(e) = Self::start_server_blocking(
                &host_for_move,
                port,
                registry,
                |actual_port, server_handle| {
                    let _ = tx.send(Ok((actual_port, server_handle)));
                },
            ) {
                let _ = tx.send(Err(e));
            }
            info!("Map server stopped");
        });
        let (actual_port, server_handle) = rx.recv()??;
        let server_info = ServerInfo {
            host: host.to_owned(),
            port: actual_port,
        };
        Ok(((handle, server_handle), server_info))
    }

    pub fn create_and_start(host: &str, port: Option<u16>) -> Result<Self> {
        let registry = Arc::new(Mutex::new(Registry::new()));

        let registry_for_move = registry.clone();
        let (handles, server_info) = Self::start_server(host, port, registry_for_move)?;

        Ok(Self {
            handles: Some(handles),
            registry,
            server_info: Arc::new(Mutex::new(server_info)),
        })
    }

    pub fn url(&self) -> String {
        let server_info = self.server_info.lock().unwrap();
        format!("http://{}:{}/", server_info.host, server_info.port)
    }

    pub fn register_map_renderer(
        &mut self,
        map_renderer: Arc<Mutex<MapRenderer>>,
    ) -> MapRendererToken {
        let id = {
            let mut registry = self.registry.lock().unwrap();
            let id = Uuid::new_v4();
            registry.insert(id, map_renderer);
            id
        };
        MapRendererToken {
            id,
            registry: Arc::downgrade(&self.registry),
            server_info: self.server_info.clone(),
            is_primitive: true,
        }
    }

    // TODO: maybe shutdown the server when switched to background
    pub fn stop(&mut self) -> Result<()> {
        if let Some((handle, server_handle)) = self.handles.take() {
            pollster::block_on(server_handle.stop(true));
            handle.join().unwrap();
        }
        Ok(())
    }

    pub fn restart(&mut self) -> Result<()> {
        let host = {
            let server_info = self.server_info.lock().unwrap();
            server_info.host.clone()
        };

        info!("Restarting server with host: {host}");

        self.stop()?;

        let registry_for_move = self.registry.clone();
        let (handles, new_server_info) = Self::start_server(&host, None, registry_for_move)?;

        {
            let mut server_info = self.server_info.lock().unwrap();
            *server_info = new_server_info;
        }

        self.handles = Some(handles);

        info!("Server successfully restarted at {}", self.url());
        Ok(())
    }

    pub fn get_ipc_test_url(&self) -> String {
        let server_info = self.server_info.lock().unwrap();
        format!("http://{}:{}", server_info.host, server_info.port)
    }

    // TODO: this is a workaround to get the registry from the map server
    // we should redesign the interface to avoid this
    pub fn get_registry(&self) -> Arc<Mutex<Registry>> {
        self.registry.clone()
    }
}

impl Drop for MapServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Journey View frontend
const JOURNEY_VIEW_HTML: &str =
    include_str!(concat!(env!("OUT_DIR"), "/journey_kernel/index.html"));

// Journey Kernel wasm package
const JOURNEY_KERNEL_JS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/journey_kernel/main.bundle.js"));

const JOURNEY_KERNEL_WASM: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/journey_kernel/journey_kernel_bg.wasm"
));

// Serve the HTML page
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(JOURNEY_VIEW_HTML)
}

async fn serve_journey_kernel_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(JOURNEY_KERNEL_JS)
}

async fn serve_journey_kernel_wasm() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/wasm")
        .body(Cow::Borrowed(JOURNEY_KERNEL_WASM))
}

async fn handle_preflight() -> HttpResponse {
    add_cors_headers(&mut HttpResponse::Ok())
        .append_header(("Access-Control-Max-Age", "86400")) // Cache preflight for 24 hours
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::internal_server::{
        Request, RequestPayload, RequestResponse, TileRangeQuery,
    };
    use serde_json;
    use std::collections::HashMap;
    use uuid::uuid;

    #[test]
    fn test_tile_range_query_roundtrip_serialization() {
        let original_query = TileRangeQuery {
            id: uuid!("25506f47-b66a-4ddc-bfbe-2a7a2ae543e3"),
            x: -999,
            y: 999,
            z: 20,
            width: 4096,
            height: 2048,
            buffer_size_power: 12,
            cached_version: Some("test-version-123".to_string()),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original_query).expect("Failed to serialize");
        assert_eq!(
            json,
            r#"{"id":"25506f47-b66a-4ddc-bfbe-2a7a2ae543e3","x":-999,"y":999,"z":20,"width":4096,"height":2048,"buffer_size_power":12,"cached_version":"test-version-123"}"#
        );

        // Deserialize back from JSON
        let deserialized_query: TileRangeQuery =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Verify all fields match
        assert_eq!(original_query.x, deserialized_query.x);
        assert_eq!(original_query.y, deserialized_query.y);
        assert_eq!(original_query.z, deserialized_query.z);
        assert_eq!(original_query.width, deserialized_query.width);
        assert_eq!(original_query.height, deserialized_query.height);
        assert_eq!(
            original_query.buffer_size_power,
            deserialized_query.buffer_size_power
        );
        assert_eq!(
            original_query.cached_version,
            deserialized_query.cached_version
        );
    }

    #[test]
    fn test_tile_range_response_roundtrip_serialization() {
        let response = TileRangeResponse {
            status: 200,
            headers: {
                let mut h = HashMap::new();
                h.insert("version".to_string(), "100".to_string());
                h
            },
            body: vec![0u8; 3],
        };

        // Serialize to JSON
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert_eq!(
            json,
            r#"{"status":200,"headers":{"version":"100"},"body":"AAAA"}"#
        );

        // Deserialize back from JSON
        let deserialized_response: TileRangeResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Verify all fields match
        assert_eq!(response.status, deserialized_response.status);
        assert_eq!(response.headers, deserialized_response.headers);
        assert_eq!(response.body, deserialized_response.body);
    }

    #[test]
    fn test_unified_json_request_parsing() {
        // Test tile range request
        let tile_request_json = r#"{
            "requestId": "test-123",
            "query": "tile_range",
            "payload": {
                "id": "25506f47-b66a-4ddc-bfbe-2a7a2ae543e3",
                "x": 0,
                "y": 0,
                "z": 0,
                "width": 1,
                "height": 1,
                "buffer_size_power": 6,
                "cached_version": null
            }
        }"#;

        let request =
            Request::parse(tile_request_json).expect("Failed to parse tile range request");
        assert_eq!(request.request_id, "test-123");
        match request.payload {
            RequestPayload::TileRange(query) => {
                assert_eq!(query.id, uuid!("25506f47-b66a-4ddc-bfbe-2a7a2ae543e3"));
                assert_eq!(query.x, 0);
                assert_eq!(query.y, 0);
                assert_eq!(query.z, 0);
                assert_eq!(query.width, 1);
                assert_eq!(query.height, 1);
                assert_eq!(query.buffer_size_power, 6);
                assert_eq!(query.cached_version, None);
            }
            _ => panic!("Expected TileRange payload"),
        }

        // Test random data request
        let random_data_request_json = r#"{
            "requestId": "random-456",
            "query": "random_data",
            "payload": {
                "size": 1024
            }
        }"#;

        let request =
            Request::parse(random_data_request_json).expect("Failed to parse random data request");
        assert_eq!(request.request_id, "random-456");
        match request.payload {
            RequestPayload::RandomData(query) => {
                assert_eq!(query.size, Some(1024));
            }
            _ => panic!("Expected RandomData payload"),
        }
    }

    #[test]
    fn test_request_response_json_serialization() {
        let response: RequestResponse<serde_json::Value> = RequestResponse {
            request_id: "test-789".to_string(),
            success: true,
            data: Some(serde_json::json!({
                "status": 200,
                "headers": {"version": "abc123"},
                "body": "AAAA"
            })),
            error: None,
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize response");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("Failed to parse response JSON");

        assert_eq!(parsed["requestId"], "test-789");
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"].is_object());
        assert!(parsed["error"].is_null());
    }
}
