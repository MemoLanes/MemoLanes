use actix_web::{
    dev::ServerHandle,
    http::{Method, StatusCode},
    web, App, HttpResponse, HttpResponseBuilder, HttpServer,
};
use anyhow::Result;
use memolanes_core::build_info;
use memolanes_core::renderer::internal_server::dispatch_request;
use memolanes_core::renderer::MapRenderer;
use std::collections::HashMap;
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
        .append_header((
            "Access-Control-Expose-Headers",
            "X-Tile-Version, X-Not-Modified",
        ))
}

async fn serve_request(
    path: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
    data: web::Data<Arc<Mutex<MapRenderer>>>,
) -> HttpResponse {
    let mut renderer = data.get_ref().lock().unwrap();
    let resp = dispatch_request(&path, &query, &mut renderer);
    let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = HttpResponse::build(status);
    add_cors_headers(&mut builder).content_type(resp.content_type);
    for (k, v) in &resp.headers {
        builder.append_header((k.as_str(), v.as_str()));
    }
    builder.body(resp.body)
}

fn parse_host_from_url(url: &str) -> Option<&str> {
    let after_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

pub fn get_dev_server_host() -> String {
    let dev_server = std::env::var("DEV_SERVER").unwrap_or_default();
    parse_host_from_url(&dev_server)
        .map(String::from)
        .unwrap_or_else(|| "localhost".to_string())
}

pub struct MapServer {
    handles: Option<(JoinHandle<()>, ServerHandle)>,
    map_renderer: Arc<Mutex<MapRenderer>>,
    port: u16,
}

impl MapServer {
    fn start_server_blocking<F>(
        map_renderer: Arc<Mutex<MapRenderer>>,
        ready_with_port: F,
    ) -> Result<()>
    where
        F: FnOnce(u16, ServerHandle),
    {
        let host = get_dev_server_host();
        let runtime = Runtime::new()?;
        runtime.block_on(async move {
            eprintln!("[INFO] Setting up map server ...");
            let data = web::Data::new(map_renderer);
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(data.clone())
                    .route("/{path}", web::get().to(serve_request))
                    .route("/{path}", web::method(Method::OPTIONS).to(handle_preflight))
            })
            .bind((host.clone(), 0))?
            .workers(1)
            .shutdown_timeout(0);

            let actual_port = if let Some(addr) = server.addrs().first() {
                addr.port()
            } else {
                return Err(anyhow::anyhow!("Failed to get server address"));
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
        map_renderer: Arc<Mutex<MapRenderer>>,
    ) -> Result<((JoinHandle<()>, ServerHandle), u16)> {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            if let Err(e) =
                Self::start_server_blocking(map_renderer, |actual_port, server_handle| {
                    let _ = tx.send(Ok((actual_port, server_handle)));
                })
            {
                let _ = tx.send(Err(e));
            }
            eprintln!("[INFO] Map server stopped");
        });
        let (actual_port, server_handle) = rx.recv()??;
        Ok(((handle, server_handle), actual_port))
    }

    pub fn create_and_start(map_renderer: Arc<Mutex<MapRenderer>>) -> Result<Self> {
        let map_renderer_clone = map_renderer.clone();
        let (handles, actual_port) = Self::start_server(map_renderer_clone)?;

        Ok(Self {
            handles: Some(handles),
            map_renderer,
            port: actual_port,
        })
    }

    pub fn get_http_url(&self) -> String {
        let dev_server =
            std::env::var("DEV_SERVER").unwrap_or_else(|_| "http://localhost:8080".to_string());

        let cgi_host = get_dev_server_host();
        let mut map_renderer = self.map_renderer.lock().unwrap();
        let bounds = map_renderer.get_map_bounds();

        match bounds {
            Some(bounds) => format!(
                "{}#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&west={}&south={}&east={}&north={}&access_key={}",
                dev_server,
                cgi_host,
                self.port,
                bounds.west,
                bounds.south,
                bounds.east,
                bounds.north,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or("")
            ),
            None => format!(
                "{}#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&access_key={}",
                dev_server,
                cgi_host,
                self.port,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or("")
            ),
        }
    }

    pub fn get_file_url(&self) -> String {
        let cgi_host = get_dev_server_host();
        let mut map_renderer = self.map_renderer.lock().unwrap();
        let bounds = map_renderer.get_map_bounds();

        match bounds {
            Some(bounds) => format!(
                "file://{}/journey_kernel/index.html#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&west={}&south={}&east={}&north={}&access_key={}",
                std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()), 
                cgi_host,
                self.port,
                bounds.west,
                bounds.south,
                bounds.east,
                bounds.north,
                build_info::MAPBOX_ACCESS_TOKEN.unwrap_or(""),
            ),
            None => format!(
                "file://{}/journey_kernel/index.html#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&access_key={}", 
                std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()), 
                cgi_host,
                self.port,
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
    use memolanes_core::journey_bitmap::JourneyBitmap;
    use memolanes_core::renderer::internal_server::dispatch_request;
    use memolanes_core::renderer::MapRenderer;
    use std::collections::HashMap;

    #[test]
    fn test_dispatch_tile_range() {
        let jb = JourneyBitmap::new();
        let mut mr = MapRenderer::new(jb);
        let params: HashMap<String, String> = [
            ("x", "0"),
            ("y", "0"),
            ("z", "0"),
            ("width", "1"),
            ("height", "1"),
            ("buffer_size_power", "6"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        let resp = dispatch_request("tile_range", &params, &mut mr);
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "application/octet-stream");
    }

    #[test]
    fn test_dispatch_random_data() {
        let jb = JourneyBitmap::new();
        let mut mr = MapRenderer::new(jb);
        let params: HashMap<String, String> = [("size", "1024")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let resp = dispatch_request("random_data", &params, &mut mr);
        assert_eq!(resp.status, 200);
        assert_eq!(resp.content_type, "application/octet-stream");
        assert_eq!(resp.body.len(), 1024);
    }

    #[test]
    fn test_dispatch_unknown_route() {
        let jb = JourneyBitmap::new();
        let mut mr = MapRenderer::new(jb);
        let params = HashMap::new();

        let resp = dispatch_request("nonexistent", &params, &mut mr);
        assert_eq!(resp.status, 500);
        assert!(String::from_utf8_lossy(&resp.body).contains("Unknown route"));
    }
}
