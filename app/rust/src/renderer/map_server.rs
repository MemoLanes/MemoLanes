use actix_web::{dev::ServerHandle, http::Method, web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::renderer::map_renderer;

use super::MapRenderer;

use rand::RngCore;

type Registry = HashMap<Uuid, Arc<Mutex<MapRenderer>>>;

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

#[derive(Deserialize, Serialize)]
struct TileRangeQuery {
    id: Uuid,
    x: i64,
    y: i64,
    z: i16,
    width: i64,
    height: i64,
    buffer_size_power: i16,
    cached_version: Option<String>,
}

}

async fn serve_journey_tile_range(
    query: web::Query<TileRangeQuery>,
    req: HttpRequest,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    let registry = data.lock().unwrap();
    match registry.get(&query.id) {
        Some(item) => {
            let map_renderer = item.lock().unwrap();

            // Extract version from If-None-Match header if present
            let client_version = query.cached_version.as_deref();

            match map_renderer.get_latest_bitmap_if_changed(client_version) {
                None => HttpResponse::NotModified().finish(),
                Some((journey_bitmap, version)) => {
                    // Create a TileBuffer from the JourneyBitmap for the specified range
                    let tile_buffer = match map_renderer::tile_buffer_from_journey_bitmap(
                        journey_bitmap,
                        query.x,
                        query.y,
                        query.z,
                        query.width,
                        query.height,
                        query.buffer_size_power,
                    ) {
                        Ok(buffer) => buffer,
                        Err(creation_error) => {
                            log::error!("Failed to create TileBuffer: {creation_error}");
                            return HttpResponse::BadRequest()
                                .content_type("text/plain")
                                .body(format!("Failed to create tile buffer: {creation_error}"));
                        }
                    };

                    // Serialize and return the TileBuffer with ETag
                    match tile_buffer.to_bytes() {
                        Ok(data) => HttpResponse::Ok()
                            .append_header(("ETag", version))
                            .content_type("application/octet-stream")
                            .body(data),
                        Err(serialization_error) => {
                            log::error!("Failed to serialize TileBuffer: {serialization_error}");
                            HttpResponse::InternalServerError()
                                .content_type("text/plain")
                                .body("Failed to serialize tile data")
                        }
                    }
                }
            }
        }
// Add this new struct for IPC test query parameters
#[derive(Deserialize, Serialize)]
struct IpcTestQuery {
    size: Option<u64>, // Size in bytes, default 1MB
}

// Move the random data generation to a separate function
pub fn generate_random_data(size: u64) -> Result<Vec<u8>, String> {
    let max_size = 10_485_760; // 10MB limit
    
    if size > max_size {
        return Err(format!("Size too large. Maximum allowed: {} bytes (10MB)", max_size));
    }
    
    // Generate random data efficiently
    let mut data = vec![0u8; size as usize];
    rand::rng().fill_bytes(&mut data);
    
    Ok(data)
}

// Simplified HTTP handler - now uses the shared function
async fn serve_ipc_test_download(query: web::Query<IpcTestQuery>) -> HttpResponse {
    let size = query.size.unwrap_or(1_048_576); // Default 1MB
    
    match generate_random_data(size) {
        Ok(data) => HttpResponse::Ok()
            .append_header(("Access-Control-Allow-Origin", "*"))
            .append_header(("Access-Control-Allow-Methods", "GET, OPTIONS"))
            .append_header(("Access-Control-Allow-Headers", "Content-Type"))
            .append_header(("Content-Length", size.to_string()))
            .content_type("application/octet-stream")
            .body(data),
        Err(error_msg) => HttpResponse::BadRequest()
            .append_header(("Access-Control-Allow-Origin", "*"))
            .append_header(("Access-Control-Allow-Methods", "GET, OPTIONS"))
            .append_header(("Access-Control-Allow-Headers", "Content-Type"))
            .content_type("text/plain")
            .body(error_msg)
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
                    .route(
                        "/tile_range",
                        web::get().to(serve_journey_tile_range),
                    )
                    .route(
                        "/tile_range",
                        web::method(Method::OPTIONS).to(handle_preflight),
                    )
                    // Only one HTTP test endpoint
                    .route("/download1M", web::get().to(serve_ipc_test_download))
                    .route("/download1M", web::method(Method::OPTIONS).to(handle_preflight))
                    .route("/", web::get().to(index))
                    .route("/main.bundle.js", web::get().to(serve_journey_kernel_js))
                    .route(
                        "/journey_kernel_bg.wasm",
                        web::get().to(serve_journey_kernel_wasm),
                    )
                    .route("/token.json", web::get().to(serve_token_json))
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
const JOURNEY_KERNEL_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/journey_kernel/main.bundle.js"));

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

async fn serve_token_json() -> HttpResponse {
    let token = env!("MAPBOX-ACCESS-TOKEN");
    let json = format!(r#"{{"MAPBOX-ACCESS-TOKEN": "{token}"}}"#);

    HttpResponse::Ok()
        .content_type("application/json")
        .body(json)
}

async fn handle_preflight() -> HttpResponse {
    HttpResponse::Ok()
        .append_header(("Access-Control-Allow-Origin", "*"))
        .append_header(("Access-Control-Allow-Methods", "GET, OPTIONS"))
        .append_header(("Access-Control-Allow-Headers", "Content-Type, If-None-Match"))
        .append_header(("Access-Control-Max-Age", "86400")) // Cache preflight for 24 hours
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use uuid::uuid;

    #[test]
    fn test_tile_range_query_roundtrip_serialization() {
        let original_query = TileRangeQuery {
            id: Some(uuid!("25506f47-b66a-4ddc-bfbe-2a7a2ae543e3")),
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
        assert_eq!(json, r#"{"id":"25506f47-b66a-4ddc-bfbe-2a7a2ae543e3","x":-999,"y":999,"z":20,"width":4096,"height":2048,"buffer_size_power":12,"cached_version":"test-version-123"}"#);
        
        // Deserialize back from JSON
        let deserialized_query: TileRangeQuery = serde_json::from_str(&json).expect("Failed to deserialize");
        
        // Verify all fields match
        assert_eq!(original_query.x, deserialized_query.x);
        assert_eq!(original_query.y, deserialized_query.y);
        assert_eq!(original_query.z, deserialized_query.z);
        assert_eq!(original_query.width, deserialized_query.width);
        assert_eq!(original_query.height, deserialized_query.height);
        assert_eq!(original_query.buffer_size_power, deserialized_query.buffer_size_power);
        assert_eq!(original_query.cached_version, deserialized_query.cached_version);
    }
}
