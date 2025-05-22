use actix_web::{dev::ServerHandle, web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Result;
use serde::{Deserialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::thread::{self, JoinHandle};
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::MapRenderer;
use crate::journey_kernel::tile_buffer::TileBuffer;

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

#[derive(Deserialize)]
struct TileRangeQuery {
    x: i64,
    y: i64,
    z: i16,
    width: i64,
    height: i64,
    buffer_size_power: i16,
}

async fn serve_journey_bitmap_by_id(
    id: web::Path<Uuid>,
    req: HttpRequest,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    let registry = data.lock().unwrap();
    match registry.get(&id) {
        Some(item) => {
            let map_renderer = item.lock().unwrap();

            // Extract version from If-None-Match header if present
            let client_version = req
                .headers()
                .get("If-None-Match")
                .and_then(|h| h.to_str().ok());

            match map_renderer.get_latest_bitmap_if_changed(client_version) {
                None => HttpResponse::NotModified().finish(),
                Some((journey_bitmap, version)) => {
                    // Check if this is a HEAD request
                    if req.method() == actix_web::http::Method::HEAD {
                        // For HEAD requests, return only the headers without the body
                        HttpResponse::Ok().append_header(("ETag", version)).finish()
                    } else {
                        // For GET requests, return the full response with body
                        match journey_bitmap.to_bytes() {
                            Ok(bytes) => HttpResponse::Ok()
                                .append_header(("ETag", version))
                                .body(bytes),
                            Err(_) => HttpResponse::InternalServerError().finish(),
                        }
                    }
                }
            }
        }
        None => HttpResponse::NotFound().finish(),
    }
}

async fn serve_journey_bitmap_provisioned_camera_option_by_id(
    id: web::Path<Uuid>,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    let registry = data.lock().unwrap();
    match registry.get(&id) {
        Some(item) => {
            let map_renderer = item.lock().unwrap();
            let camera_option = map_renderer.get_provisioned_camera_option();
            HttpResponse::Ok().json(camera_option)
        }
        None => HttpResponse::NotFound().finish(),
    }
}

async fn serve_journey_tile_range(
    id: web::Path<Uuid>,
    query: web::Query<TileRangeQuery>,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    let registry = data.lock().unwrap();
    match registry.get(&id) {
        Some(item) => {
            let map_renderer = item.lock().unwrap();
            let journey_bitmap = map_renderer.peek_latest_bitmap();

            // Create a TileBuffer from the JourneyBitmap for the specified range
            let tile_buffer = TileBuffer::from_journey_bitmap(
                journey_bitmap,
                query.x,
                query.y,
                query.z,
                query.width,
                query.height,
                query.buffer_size_power,
            );

            // Serialize and return the TileBuffer
            match tile_buffer.to_bytes() {
                Ok(data) => HttpResponse::Ok()
                    .content_type("application/octet-stream")
                    .body(data),
                Err(_) => HttpResponse::InternalServerError().finish(),
            }
        }
        None => HttpResponse::NotFound().finish(),
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
                        "/journey/{id}/journey_bitmap.bin",
                        web::get().to(serve_journey_bitmap_by_id),
                    )
                    .route(
                        "/journey/{id}/journey_bitmap.bin",
                        web::head().to(serve_journey_bitmap_by_id),
                    )
                    .route(
                        "/journey/{id}/provisioned_camera_option",
                        web::get().to(serve_journey_bitmap_provisioned_camera_option_by_id),
                    )
                    .route(
                        "/journey/{id}/tile_range",
                        web::get().to(serve_journey_tile_range),
                    )
                    .route("/", web::get().to(index))
                    .route("/bundle.js", web::get().to(serve_journey_kernel_js))
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
            info!("Server bound successfully to {}:{}", host, actual_port);

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

        info!("Restarting server with host: {}", host);

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
const JOURNEY_KERNEL_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/journey_kernel/bundle.js"));

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
