use crate::journey_bitmap::JourneyBitmap;
use actix_web::dev::Service;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use journey_kernel::journey_bitmap;
use std::borrow::Cow;
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use tokio::runtime::Runtime;
use uuid::Uuid;
use std::collections::HashMap;

// TODO:: maybe we should move this out of the api
use crate::api::api::CameraOption;

use super::MapRenderer;

// App state holding the registry
struct State {
    registry: HashMap<Uuid, Arc<Mutex<MapRenderer>>>,
    // TODO: The option for map_renderer is weird, we should fix it.
    map_renderer: Arc<Mutex<Option<MapRenderer>>>,
    provisioned_camera_option: Option<CameraOption>,
}

pub struct MapRendererToken {
    id: Uuid,
    url: String,
    state: Weak<Mutex<State>>,
}

impl MapRendererToken {
    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn get_map_renderer(&self) -> Option<Arc<Mutex<MapRenderer>>> {
        if let Some(state) = self.state.upgrade() {
            let state = state.lock().unwrap();
            return state.registry.get(&self.id).cloned();
        }
        None
    }

    pub fn unregister(&self) {
        if let Some(state) = self.state.upgrade() {
            let mut state = state.lock().unwrap();
            state.registry.remove(&self.id);
        }
    }
}

impl Drop for MapRendererToken {
    fn drop(&mut self) {
        self.unregister();
    }
}

// Handler for serving registered items
async fn serve_main_journey_bitmap(
    req: HttpRequest,
    data: web::Data<Arc<Mutex<State>>>,
) -> HttpResponse {
    let state = data.lock().unwrap();
    let mut map_renderer = state.map_renderer.lock().unwrap();

    match map_renderer.as_mut() {
        None => HttpResponse::NotFound().finish(),
        Some(map_renderer) => {
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
                    Err(_) => return HttpResponse::InternalServerError().finish(),
                },
            }
        }
    }
}

async fn serve_journey_bitmap_by_id(
    id: web::Path<String>,
    req: HttpRequest,
    data: web::Data<Arc<Mutex<State>>>,
) -> HttpResponse {
    info!("serving item: {}", id);
    let state = data.lock().unwrap();

    // Parse UUID and get item from registry
    match Uuid::parse_str(&id)
        .ok()
        .and_then(|uuid| state.registry.get(&uuid))
        {
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
                        Err(_) => return HttpResponse::InternalServerError().finish(),
                    },
                }
            },
            None => HttpResponse::NotFound().finish(),
        }
}

async fn serve_main_journey_bitmap_provisioned_camera_option(
    data: web::Data<Arc<Mutex<State>>>,
) -> HttpResponse {
    let state = data.lock().unwrap();
    let camera_option = state.provisioned_camera_option;
    HttpResponse::Ok().json(camera_option)
}

pub struct MapServer {
    handle: Option<thread::JoinHandle<()>>,
    state: Arc<Mutex<State>>,
    url: Arc<Mutex<String>>,
}

impl MapServer {
    pub fn create_and_start(host: &str, port: u16) -> std::io::Result<Self> {
        let mut server = Self {
            handle: None,
            state: Arc::new(Mutex::new(State {
                registry: HashMap::new(),
                map_renderer: Arc::new(Mutex::new(None)),
                provisioned_camera_option: None,
            })),
            url: Arc::new(Mutex::new(String::new())),
        };
        server.start(host, port)?;
        Ok(server)
    }

    pub fn get_url(&self) -> String {
        self.url.lock().unwrap().clone()
    }

    // Start the server in a separate thread
    fn start(&mut self, host: &str, port: u16) -> std::io::Result<()> {
        let host = host.to_string();
        let mut port = port;
        let random_prefix = Uuid::new_v4().to_string();
        let random_prefix2 = random_prefix.clone();

        let url = self.url.clone();

        // Create a channel to signal when the URL prefix is set
        let (tx, rx) = std::sync::mpsc::channel();

        let data = web::Data::new(self.state.clone());
        let handle = thread::spawn(move || {
            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime.block_on(async move {
                info!("Setting up server routes...");
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(data.clone())
                        .wrap_fn(|req, srv| {
                            info!("Incoming request: {} {}", req.method(), req.uri());
                            srv.call(req)
                        })
                        .route(
                            &format!("/{}/journey_bitmap.bin", random_prefix),
                            web::get().to(serve_main_journey_bitmap),
                        )
                        .route(
                            &format!("/{}/journey/{{id}}/journey_bitmap.bin", random_prefix),
                            web::get().to(serve_journey_bitmap_by_id),
                        )
                        .route(
                            &format!("/{}/provisioned_camera_option", random_prefix),
                            web::get().to(serve_main_journey_bitmap_provisioned_camera_option),
                        )
                        .route(&format!("/{}/", random_prefix), web::get().to(index))
                        .route(
                            &format!("/{}/bundle.js", random_prefix),
                            web::get().to(serve_journey_kernel_js),
                        )
                        .route(
                            &format!("/{}/journey_kernel_bg.wasm", random_prefix),
                            web::get().to(serve_journey_kernel_wasm),
                        )
                        .route(
                            &format!("/{}/token.json", random_prefix).to_string(),
                            web::get().to(serve_token_json),
                        )
                })
                .bind(format!("{}:{}", host, port))
                .expect("Failed to bind server");

                // If port was 0, get the actual port and update registry's URL prefix
                if port == 0 {
                    if let Some(addr) = server.addrs().first() {
                        let actual_port = addr.port();
                        port = actual_port;
                    }
                }

                {
                    let mut url_mut = url.lock().unwrap();
                    *url_mut = format!("http://{}:{}/{}/", host, port, random_prefix2);
                }

                // Signal that the URL prefix is set
                tx.send(()).expect("Failed to send completion signal");

                info!("Server bound successfully to {}:{}", host, port);
                server.run().await.expect("Server failed to run");
            });
        });

        // Wait for the URL prefix to be set
        rx.recv().expect("Failed to receive completion signal");

        self.handle = Some(handle);
        Ok(())
    }

    pub fn set_map_renderer(
        &mut self,
        //TODO: Again, the option here is weird.
        map_renderer: Arc<Mutex<Option<MapRenderer>>>,
        provisioned_camera_option: Option<CameraOption>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.map_renderer = map_renderer;
        state.provisioned_camera_option = provisioned_camera_option;
    }

    pub fn register_map_renderer(&mut self, map_renderer: Arc<Mutex<MapRenderer>>) -> MapRendererToken {
        let mut state = self.state.lock().unwrap();
        let id = Uuid::new_v4();
        state.registry.insert(id, map_renderer);
        MapRendererToken {
            id,
            url: format!("{}#journey_id={}", self.get_url(), id),
            state: Arc::downgrade(&self.state),
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
