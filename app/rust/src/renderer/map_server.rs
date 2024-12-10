use crate::journey_bitmap::JourneyBitmap;
use actix_web::{web, App, HttpResponse, HttpServer};
use std::borrow::Cow;
use std::thread;
use tokio::runtime::Runtime;
use uuid::Uuid;
// TODO: check whether to use tokio's sync library
use actix_web::dev::Service;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

// TODO:: maybe we should move this out of the api
use crate::api::api::CameraOption;


// TODO: implement the push-based map command interface
/// the Token is owned by flutter side code whenever there is a webview based map widget to be created.
/// token can be resolved into a uri that can be used to access the journey bitmap
/// token also provides interface for setting the camera options and pushing map commands to the map frontend
/// the web availablity to the journey bitmap is preserved until the token is dropped (or the journey bitmap itself is no longer accessible)
pub struct Token {
    id: Uuid,
    url: String,
    provisioned_camera_option: Arc<Mutex<Option<CameraOption>>>,
    registry: Weak<RwLock<HashMap<Uuid, JourneyBitmapEntry>>>,
}

impl Token {
    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn set_provisioned_camera_option(&self, camera_option: CameraOption) {
        let mut provisioned_camera_option = self.provisioned_camera_option.lock().unwrap();
        *provisioned_camera_option = Some(camera_option);
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            println!("dropping entry: {} from registry", self.id);
            let mut items = registry.write().unwrap();
            items.remove(&self.id);
        } else {
            println!("registry not found for token: {}", self.id);
        }
    }
}

/// A wrapper for journey bitmap reference stored in the server's registry
/// provisioned camera option: the backend can provide a default camera setting the for map, which depends on the use case, can either be:
///     1. a general view of the whole journey
///     2. current location of the user
/// map commands queue: the rust side can also push commands to the map frontend, which will be executed in sequence
pub struct JourneyBitmapEntry {
    journey_bitmap: Weak<Mutex<JourneyBitmap>>,
    provisioned_camera_option: Arc<Mutex<Option<CameraOption>>>,
    // map_commands_queue: Arc<Mutex<Vec<MapCommand>>>,
}

// Registry system for any serializable type
#[derive(Default, Clone)]
pub struct Registry {
    url_prefix: Arc<RwLock<String>>,
    items: Arc<RwLock<HashMap<Uuid, JourneyBitmapEntry>>>,
}

impl Registry {
    pub fn new(url_prefix: &str) -> Self {
        Self {
            url_prefix: Arc::new(RwLock::new(url_prefix.to_string())),
            items: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn set_url_prefix(&self, url_prefix: &str) {
        let mut prefix = self.url_prefix.write().unwrap();
        *prefix = url_prefix.to_string();
    }

    pub fn register(&self, item: Weak<Mutex<JourneyBitmap>>) -> Token {
        let id = Uuid::new_v4();
        let provisioned_camera_option = Arc::new(Mutex::new(None));
        {
            let mut items = self.items.write().unwrap();
            let entry = JourneyBitmapEntry {
                journey_bitmap: item,
                provisioned_camera_option: provisioned_camera_option.clone(),
            };
            items.insert(id, entry);
        }
        let url_prefix = self.url_prefix.read().unwrap();
        Token {
            id,
            url: format!("{}/#{}", *url_prefix, id),
            provisioned_camera_option,
            registry: Arc::downgrade(&self.items),
        }
    }

    pub fn get(&self, id: &Uuid) -> Option<Weak<Mutex<JourneyBitmap>>> {
        let items = self.items.read().unwrap();
        items.get(id).map(|entry| entry.journey_bitmap.clone())
    }

    pub fn get_provisioned_camera_option(&self, id: &Uuid) -> Option<CameraOption> {
        let items = self.items.read().unwrap();
        if let Some(entry) = items.get(id) {
            let camera_option = entry.provisioned_camera_option.lock().unwrap().clone();
            camera_option
        } else {
            None
        }
    }
}

// App state holding the registry
struct AppState {
    registry: Registry,
}

// Handler for serving registered items
async fn serve_item(id: web::Path<String>, data: web::Data<AppState>) -> HttpResponse {
    println!("serving item: {}", id);
    match Uuid::parse_str(&id)
        .ok()
        .and_then(|uuid| data.registry.get(&uuid))
    {
        Some(item) => match item.upgrade() {
            Some(journey_bitmap) => match journey_bitmap.lock().unwrap().to_bytes() {
                Ok(bytes) => HttpResponse::Ok().body(bytes),
                Err(_) => HttpResponse::InternalServerError().finish(),
            },
            None => HttpResponse::NotFound().finish(),
        },
        None => HttpResponse::NotFound().finish(),
    }
}

async fn serve_provisioned_camera_option(
    id: web::Path<String>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let camera_option = data
        .registry
        .get_provisioned_camera_option(&Uuid::parse_str(&id).unwrap());
    HttpResponse::Ok().json(camera_option)
}


pub struct MapServer {
    host: String,
    port: u16,
    #[allow(dead_code)]
    runtime: Option<Runtime>,
    handle: Option<thread::JoinHandle<()>>,
    registry: Arc<Registry>,
}

impl MapServer {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            runtime: None,
            handle: None,
            registry: Arc::new(Registry::new(&format!("http://{}:{}", host, port))),
        }
    }

    pub fn register(&self, item: Weak<Mutex<JourneyBitmap>>) -> Token {
        self.registry.register(item)
    }

    // Start the server in a separate thread
    pub fn start(&mut self) -> std::io::Result<()> {
        let host = self.host.clone();
        let mut port = self.port;
        let registry = self.registry.clone();
        let random_prefix = Uuid::new_v4().to_string();
        let random_prefix2 = random_prefix.clone();

        let handle = thread::spawn(move || {
            let app_state = web::Data::new(AppState {
                registry: (*registry).clone(),
            });

            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime.block_on(async move {
                println!("Setting up server routes...");
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(app_state.clone())
                        .wrap_fn(|req, srv| {
                            println!("Incoming request: {} {}", req.method(), req.uri());
                            srv.call(req)
                        })
                        .route(
                            &format!("/{}/items/{{id}}", random_prefix),
                            web::get().to(serve_item),
                        )
                        .route(
                            &format!("/{}/items/{{id}}/provisioned_camera_option", random_prefix),
                            web::get().to(serve_provisioned_camera_option),
                        )
                        .route(&format!("/{}/", random_prefix), web::get().to(index))
                        .route(
                            "/pkg/journey_kernel.js",
                            web::get().to(serve_journey_kernel_js),
                        )
                        .route(
                            "/pkg/journey_kernel_bg.wasm",
                            web::get().to(serve_journey_kernel_wasm),
                        )
                        .route(
                            &format!("/{}/mapbox-gl.js", random_prefix).to_string(),
                            web::get().to(serve_mapbox_js),
                        )
                        .route(
                            &format!("/{}/mapbox-gl.css", random_prefix).to_string(),
                            web::get().to(serve_mapbox_css),
                        )
                        .route(
                            &format!("/{}/token.json", random_prefix).to_string(),
                            web::get().to(serve_token_json),
                        )
                        .route(
                            &format!("/{}/journey-canvas-layer.js", random_prefix).to_string(),
                            web::get().to(serve_journey_canvas_layer_js),
                        )
                })
                .bind(format!("{}:{}", host, port))
                .expect("Failed to bind server");

                // If port was 0, get the actual port and update registry's URL prefix
                if port == 0 {
                    if let Some(addr) = server.addrs().first() {
                        let actual_port = addr.port();
                        port = actual_port;
                        // println!("Server bound to dynamic port: {}", actual_port);
                    }
                }

                registry.set_url_prefix(&format!("http://{}:{}/{}", host, port, random_prefix2));

                // println!("Server bound successfully to {}:{}", host, port);
                server.run().await.expect("Server failed to run");
            });
        });

        self.handle = Some(handle);
        Ok(())
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
const JOURNEY_VIEW_HTML: &str = include_str!("../../../journey_kernel/examples/journey-view.html");
const JOURNEY_CANVAS_LAYER_JS: &str =
    include_str!("../../../journey_kernel/examples/journey-canvas-layer.js");

// Journey Kernel wasm package
const JOURNEY_KERNEL_JS: &str = include_str!("../../../journey_kernel/pkg/journey_kernel.js");
const JOURNEY_KERNEL_WASM: &[u8] =
    include_bytes!("../../../journey_kernel/pkg/journey_kernel_bg.wasm");

// Mapbox
const MAPBOX_JS: &str = include_str!("../../../journey_kernel/examples/mapbox-gl.js");
const MAPBOX_CSS: &str = include_str!("../../../journey_kernel/examples/mapbox-gl.css");
const TOKEN_JSON: &str = include_str!("../../../journey_kernel/examples/token.json");


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

async fn serve_mapbox_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(MAPBOX_JS)
}

async fn serve_mapbox_css() -> HttpResponse {
    HttpResponse::Ok().content_type("text/css").body(MAPBOX_CSS)
}

async fn serve_token_json() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(TOKEN_JSON)
}

async fn serve_journey_canvas_layer_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(JOURNEY_CANVAS_LAYER_JS)
}

