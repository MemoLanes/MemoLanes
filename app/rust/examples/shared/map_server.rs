use actix_web::{
    dev::ServerHandle, http::Method, web, App, HttpResponse, HttpResponseBuilder, HttpServer,
};
use anyhow::Result;
use memolanes_core::build_info;
use memolanes_core::renderer::internal_server::{handle_tile_range_query, Request, TileRangeQuery};
use memolanes_core::renderer::{get_default_camera_option_from_journey_bitmap, MapRenderer};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use tokio::runtime::Runtime;

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
    data: web::Data<Arc<Mutex<MapRenderer>>>,
) -> HttpResponse {
    let mut map_renderer = data.get_ref().lock().unwrap();
    match handle_tile_range_query(&query.into_inner(), &mut map_renderer) {
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
        Err(error_message) => add_cors_headers(&mut HttpResponse::InternalServerError())
            .content_type("text/plain")
            .body(error_message),
    }
}

// Unified JSON POST endpoint that handles all request types
async fn serve_unified_json_request(
    body: web::Bytes,
    data: web::Data<Arc<Mutex<MapRenderer>>>,
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
                .body(format!(r#"{{"error": "Failed to parse request: {e}"}}"#));
        }
    };

    // Handle the request using the unified interface
    let mut map_renderer = data.get_ref().lock().unwrap();
    let response = request.handle(&mut map_renderer);

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
                r#"{{"error": "Failed to serialize response: {e}"}}"#
            )),
    }
}

pub struct ServerInfo {
    pub host: String,
    pub port: u16,
}

pub struct MapServer {
    handles: Option<(JoinHandle<()>, ServerHandle)>,
    map_renderer: Arc<Mutex<MapRenderer>>,
    server_info: Arc<Mutex<ServerInfo>>,
}

impl MapServer {
    fn start_server_blocking<F>(
        host: &str,
        port: Option<u16>,
        map_renderer: Arc<Mutex<MapRenderer>>,
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
            eprintln!("[INFO] Setting up map server ...");
            let data = web::Data::new(map_renderer);
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
                        return Err(anyhow::anyhow!("Failed to get server address"));
                    }
                }
            };
            eprintln!("[INFO] Server bound successfully to {host}:{actual_port}");

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
        map_renderer: Arc<Mutex<MapRenderer>>,
    ) -> Result<((JoinHandle<()>, ServerHandle), ServerInfo)> {
        let (tx, rx) = std::sync::mpsc::channel();
        let host_for_move = host.to_owned();
        let handle = thread::spawn(move || {
            if let Err(e) = Self::start_server_blocking(
                &host_for_move,
                port,
                map_renderer,
                |actual_port, server_handle| {
                    let _ = tx.send(Ok((actual_port, server_handle)));
                },
            ) {
                let _ = tx.send(Err(e));
            }
            eprintln!("[INFO] Map server stopped");
        });
        let (actual_port, server_handle) = rx.recv()??;
        let server_info = ServerInfo {
            host: host.to_owned(),
            port: actual_port,
        };
        Ok(((handle, server_handle), server_info))
    }

    pub fn create_and_start(
        host: &str,
        port: Option<u16>,
        map_renderer: Arc<Mutex<MapRenderer>>,
    ) -> Result<Self> {
        let map_renderer_clone = map_renderer.clone();
        let (handles, server_info) = Self::start_server(host, port, map_renderer_clone)?;

        Ok(Self {
            handles: Some(handles),
            map_renderer,
            server_info: Arc::new(Mutex::new(server_info)),
        })
    }

    pub fn get_http_url(&self) -> String {
        let dev_server =
            std::env::var("DEV_SERVER").unwrap_or_else(|_| "http://localhost:8080".to_string());

        let server_info = self.server_info.lock().unwrap();
        let map_renderer = self.map_renderer.lock().unwrap();
        let camera_option =
            get_default_camera_option_from_journey_bitmap(map_renderer.peek_latest_bitmap());

        match camera_option {
            Some(camera) => format!(
                "{}#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&lng={}&lat={}&zoom={}&access_key={}",
                dev_server,
                server_info.host,
                server_info.port,
                camera.lng,
                camera.lat,
                camera.zoom,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or("")
            ),
            None => format!(
                "{}#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&access_key={}",
                dev_server,
                server_info.host,
                server_info.port,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or("")
            ),
        }
    }

    pub fn get_file_url(&self) -> String {
        let server_info = self.server_info.lock().unwrap();
        let map_renderer = self.map_renderer.lock().unwrap();
        let camera_option =
            get_default_camera_option_from_journey_bitmap(map_renderer.peek_latest_bitmap());

        match camera_option {
            Some(camera) => format!(
                "file://{}/journey_kernel/index.html#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&lng={}&lat={}&zoom={}&access_key={}", 
                std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()), 
                server_info.host,
                server_info.port,
                camera.lng,
                camera.lat,
                camera.zoom,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or(""),
            ),
            None => format!(
                "file://{}/journey_kernel/index.html#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&access_key={}", 
                std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()), 
                server_info.host,
                server_info.port,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or(""),
            )
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
}

impl Drop for MapServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

async fn handle_preflight() -> HttpResponse {
    add_cors_headers(&mut HttpResponse::Ok())
        .append_header(("Access-Control-Max-Age", "86400")) // Cache preflight for 24 hours
        .finish()
}

#[cfg(test)]
mod tests {
    use memolanes_core::renderer::internal_server::{
        Request, RequestPayload, RequestResponse, TileRangeQuery, TileRangeResponse,
    };
    use serde_json;
    use std::collections::HashMap;

    #[test]
    fn test_tile_range_query_roundtrip_serialization() {
        let original_query = TileRangeQuery {
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
            r#"{"x":-999,"y":999,"z":20,"width":4096,"height":2048,"buffer_size_power":12,"cached_version":"test-version-123"}"#
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
