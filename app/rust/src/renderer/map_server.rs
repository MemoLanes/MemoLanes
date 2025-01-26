use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::Result;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::MapRenderer;

type Registry = HashMap<Uuid, Arc<Mutex<MapRenderer>>>;

pub struct MapRendererToken {
    id: Uuid,
    url: String,
    registry: Weak<Mutex<Registry>>,
    is_primitive: bool,
}

impl MapRendererToken {
    pub fn url(&self) -> String {
        self.url.clone()
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
            url: self.url.clone(),
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

async fn serve_journey_bitmap_by_id(
    id: web::Path<Uuid>,
    req: HttpRequest,
    data: web::Data<Arc<Mutex<Registry>>>,
) -> HttpResponse {
    let registry = data.lock().unwrap();
    match registry.get(&id) {
        Some(item) => {
            let mut map_renderer = item.lock().unwrap();

            // we will return 304 if the request is conditional, and the remote joruney_bitmap is up-to-date.
            let is_request_conditional = req
                .headers()
                .get("If-None-Match")
                .and_then(|h| h.to_str().ok())
                .map(|s| s == "*")
                .unwrap_or(false);
            if !is_request_conditional {
                // for non-conditional request, we always send the latest journey_bitmap
                map_renderer.reset();
            }
            match map_renderer.get_latest_bitmap_if_changed() {
                None => HttpResponse::NotModified().finish(),
                Some(journey_bitmap) => match journey_bitmap.to_bytes() {
                    Ok(bytes) => HttpResponse::Ok().body(bytes),
                    Err(_) => HttpResponse::InternalServerError().finish(),
                },
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

pub struct MapServer {
    handle: Option<thread::JoinHandle<()>>,
    registry: Arc<Mutex<Registry>>,
    url: String,
}

impl MapServer {
    fn start_server_blocking<F>(
        host: &str,
        port: Option<u16>,
        registry: Arc<Mutex<Registry>>,
        ready_with_url: F,
    ) -> Result<()>
    where
        F: FnOnce(String),
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
                        "/journey/{id}/provisioned_camera_option",
                        web::get().to(serve_journey_bitmap_provisioned_camera_option_by_id),
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
            .workers(1);

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
            ready_with_url(format!("http://{}:{}/", host, actual_port));
            // the error below is just ignored but we don't care a lot
            server.run().await?;
            Ok(())
        })
    }

    pub fn create_and_start(host: &str, port: Option<u16>) -> Result<Self> {
        let registry = Arc::new(Mutex::new(Registry::new()));

        // Create a channel to get the URL when the server is ready
        let (tx, rx) = std::sync::mpsc::channel();

        let host = host.to_owned();
        let registry_for_move = registry.clone();
        let handle = thread::spawn(move || {
            match Self::start_server_blocking(&host, port, registry_for_move, |url| {
                let _ = tx.send(Ok(url));
            }) {
                Ok(()) => (),
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
            info!("Map server stopped");
        });

        // wait for the first message from the channel, either the URL or an error
        let url = rx.recv()??;

        Ok(Self {
            handle: Some(handle),
            registry,
            url,
        })
    }

    pub fn get_url(&self) -> String {
        self.url.clone()
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
            url: format!("{}#journey_id={}", self.get_url(), id),
            registry: Arc::downgrade(&self.registry),
            is_primitive: true,
        }
    }

    // TODO: maybe stop the server when app is switched to background.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // You might want to implement proper shutdown logic here
            // For now, we just wait for the thread to finish
            handle.join().unwrap();
        }
    }
}

impl Drop for MapServer {
    fn drop(&mut self) {
        self.stop();
    }
}

// Journey View frontend
const JOURNEY_VIEW_HTML: &str = include_str!("../../../journey_kernel/dist/index.html");

// Journey Kernel wasm package
const JOURNEY_KERNEL_JS: &str = include_str!("../../../journey_kernel/dist/bundle.js");
const JOURNEY_KERNEL_WASM: &[u8] =
    include_bytes!("../../../journey_kernel/dist/journey_kernel_bg.wasm");

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
