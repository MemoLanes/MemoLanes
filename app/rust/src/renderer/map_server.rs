use crate::journey_bitmap::JourneyBitmap;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use std::borrow::Cow;
use std::thread;
use tokio::runtime::Runtime;
use uuid::Uuid;
// TODO: check whether to use tokio's sync library
use actix_web::dev::Service;
use std::sync::{Arc, Mutex, Weak};

// TODO:: maybe we should move this out of the api
use crate::api::api::CameraOption;

type JourneyBitmapModifier = Box<dyn Fn(&mut JourneyBitmap) -> bool + Send + Sync>;

// App state holding the registry
struct AppState {
    journey_bitmap: Arc<Mutex<Option<Weak<Mutex<JourneyBitmap>>>>>,
    provisioned_camera_option: Arc<Mutex<Option<CameraOption>>>,
    needs_reload: Arc<Mutex<bool>>,
    poll_handler: Arc<Mutex<Option<JourneyBitmapModifier>>>,
}

// Handler for serving registered items
async fn serve_main_journey_bitmap(req: HttpRequest, data: web::Data<AppState>) -> HttpResponse {
    info!("serving main journey bitmap");

    // see if a journey_bitmap is available
    if let Some(weak_journey_bitmap) = data.journey_bitmap.lock().unwrap().clone() {
        // we will first make sure the local journey_bitmap copy is the latest.
        let mut needs_reload = false;

        // poll the poll-handler if available to see if reconstruct the journey_bitmap is necessary
        //  (eg. the mainmap may need to be refreshed if a journey-delete-operation has been made.)
        if let Some(poll_handler) = &data.poll_handler.lock().unwrap().as_ref() {
            if let Some(strong_ref) = weak_journey_bitmap.upgrade() {
                if let Ok(mut guard) = strong_ref.lock() {
                    // Get a mutable reference from the MutexGuard
                    let mut_journey_bitmap: &mut JourneyBitmap = &mut guard;

                    // Call the function
                    needs_reload = needs_reload || poll_handler(mut_journey_bitmap);
                }
            } else {
                return HttpResponse::NotFound().finish();
            }
        }

        // check if the journey_bitmap is updated elsewhere (eg. on_location_update will add_line to the journey_bitmap)
        needs_reload = needs_reload || *data.needs_reload.lock().unwrap();

        // we will return 304 if the request is conditional, and the remote joruney_bitmap is up-to-date.
        let is_request_conditional = req
            .headers()
            .get("If-None-Match")
            .and_then(|h| h.to_str().ok())
            .map(|s| s == "*")
            .unwrap_or(false);

        if is_request_conditional && !needs_reload {
            return HttpResponse::NotModified().finish();
        }

        let mut needs_reload = data.needs_reload.lock().unwrap();
        *needs_reload = false;

        let response = match weak_journey_bitmap.upgrade() {
            Some(strong_ref) => match strong_ref.lock().unwrap().to_bytes() {
                Ok(bytes) => HttpResponse::Ok().body(bytes),
                Err(_) => return HttpResponse::InternalServerError().finish(),
            },
            None => return HttpResponse::NotFound().finish(),
        };

        response
    } else {
        return HttpResponse::NotFound().finish();
    }
}

async fn serve_main_journey_bitmap_provisioned_camera_option(
    data: web::Data<AppState>,
) -> HttpResponse {
    let camera_option = data.provisioned_camera_option.lock().unwrap().clone();
    HttpResponse::Ok().json(camera_option)
}

pub struct MapServer {
    host: String,
    port: u16,
    #[allow(dead_code)]
    runtime: Option<Runtime>,
    handle: Option<thread::JoinHandle<()>>,
    journey_bitmap: Arc<Mutex<Option<Weak<Mutex<JourneyBitmap>>>>>,
    provisioned_camera_option: Arc<Mutex<Option<CameraOption>>>,
    needs_reload: Arc<Mutex<bool>>,
    poll_handler: Arc<Mutex<Option<JourneyBitmapModifier>>>,
    url: Arc<Mutex<String>>,
}

impl MapServer {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            runtime: None,
            handle: None,
            journey_bitmap: Arc::new(Mutex::new(None)),
            provisioned_camera_option: Arc::new(Mutex::new(None)),
            needs_reload: Arc::new(Mutex::new(false)),
            poll_handler: Arc::new(Mutex::new(None)),
            url: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn set_journey_bitmap(&self, item: Weak<Mutex<JourneyBitmap>>) {
        // clear previous provisioned_camera_option
        {
            let mut provisioned_camera_option = self.provisioned_camera_option.lock().unwrap();
            *provisioned_camera_option = None;
        }

        // set journey_bitmap and poll_handler
        {
            let mut journey_bitmap = self.journey_bitmap.lock().unwrap();
            *journey_bitmap = Some(item);
        }
        {
            let mut poll_handler = self.poll_handler.lock().unwrap();
            *poll_handler = None;
        }

        // set needs_reload
        {
            let mut needs_reload = self.needs_reload.lock().unwrap();
            *needs_reload = true;
        }
    }

    pub fn set_journey_bitmap_with_poll_handler(
        &self,
        item: Weak<Mutex<JourneyBitmap>>,
        handler: impl Fn(&mut JourneyBitmap) -> bool + Send + Sync + 'static,
    ) {
        // clear previous provisioned_camera_option
        {
            let mut provisioned_camera_option = self.provisioned_camera_option.lock().unwrap();
            *provisioned_camera_option = None;
        }

        // set journey_bitmap and poll_handler
        {
            let mut journey_bitmap = self.journey_bitmap.lock().unwrap();
            *journey_bitmap = Some(item);
        }
        {
            let mut poll_handler = self.poll_handler.lock().unwrap();
            *poll_handler = Some(Box::new(handler));
        }

        // set needs_reload
        {
            let mut needs_reload = self.needs_reload.lock().unwrap();
            *needs_reload = true;
        }
    }

    pub fn set_needs_reload(&self) {
        let mut needs_reload = self.needs_reload.lock().unwrap();
        *needs_reload = true;
    }

    pub fn set_provisioned_camera_option(&self, camera_option: Option<CameraOption>) {
        let mut provisioned_camera_option = self.provisioned_camera_option.lock().unwrap();
        *provisioned_camera_option = camera_option;
    }

    pub fn get_url(&self) -> String {
        self.url.lock().unwrap().clone()
    }

    // Start the server in a separate thread
    pub fn start(&mut self) -> std::io::Result<()> {
        let host = self.host.clone();
        let mut port = self.port;
        // let registry = self.registry.clone();
        let random_prefix = Uuid::new_v4().to_string();
        let random_prefix2 = random_prefix.clone();

        let url = self.url.clone();

        // Create a channel to signal when the URL prefix is set
        let (tx, rx) = std::sync::mpsc::channel();

        let journey_bitmap = self.journey_bitmap.clone();
        let provisioned_camera_option = self.provisioned_camera_option.clone();
        let needs_reload = self.needs_reload.clone();
        let poll_handler = self.poll_handler.clone();

        let handle = thread::spawn(move || {
            let app_state = web::Data::new(AppState {
                // registry: (*registry).clone(),
                journey_bitmap,
                provisioned_camera_option,
                needs_reload,
                poll_handler,
            });

            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime.block_on(async move {
                info!("Setting up server routes...");
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(app_state.clone())
                        .wrap_fn(|req, srv| {
                            info!("Incoming request: {} {}", req.method(), req.uri());
                            srv.call(req)
                        })
                        .route(
                            &format!("/{}/items/main_journey_bitmap", random_prefix),
                            web::get().to(serve_main_journey_bitmap),
                        )
                        .route(
                            &format!(
                                "/{}/items/main_journey_bitmap/provisioned_camera_option",
                                random_prefix
                            ),
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
                    *url_mut = format!(
                        "http://{}:{}/{}/#main_journey_bitmap",
                        host, port, random_prefix2
                    );
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
