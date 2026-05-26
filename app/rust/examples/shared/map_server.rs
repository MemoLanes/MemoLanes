use actix_web::{
    dev::ServerHandle, http::Method, web, App, HttpResponse, HttpResponseBuilder, HttpServer,
};
use anyhow::Result;
use memolanes_core::build_info;
use memolanes_core::renderer::internal_server::{
    generate_random_data, handle_tile_range_query, RandomDataQuery, TileRangeQuery,
};
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
        .append_header(("Access-Control-Expose-Headers", "X-Tile-Version"))
}

async fn serve_journey_tile_range(
    query: web::Query<TileRangeQuery>,
    data: web::Data<Arc<Mutex<MapRenderer>>>,
) -> HttpResponse {
    let mut map_renderer = data.get_ref().lock().unwrap();
    match handle_tile_range_query(&query.into_inner(), &mut map_renderer) {
        Ok(tile_response) => match tile_response.status {
            200 => {
                let mut builder = HttpResponse::Ok();
                add_cors_headers(&mut builder).content_type("application/octet-stream");
                if let Some(version) = tile_response.headers.get("version") {
                    builder.append_header(("X-Tile-Version", version.as_str()));
                }
                builder.body(tile_response.body)
            }
            304 => add_cors_headers(&mut HttpResponse::NotModified()).finish(),
            _ => add_cors_headers(&mut HttpResponse::InternalServerError())
                .content_type("text/plain")
                .body(format!("Unexpected status: {}", tile_response.status)),
        },
        Err(error_message) => add_cors_headers(&mut HttpResponse::InternalServerError())
            .content_type("text/plain")
            .body(error_message),
    }
}

async fn serve_random_data(query: web::Query<RandomDataQuery>) -> HttpResponse {
    let size = query.size.unwrap_or(1_048_576);
    match generate_random_data(size) {
        Ok(data) => {
            let json = serde_json::json!({
                "success": true,
                "data": { "size": size },
            });
            add_cors_headers(&mut HttpResponse::Ok())
                .content_type("application/json")
                .body(json.to_string())
        }
        Err(e) => {
            let json = serde_json::json!({
                "success": false,
                "error": e,
            });
            add_cors_headers(&mut HttpResponse::InternalServerError())
                .content_type("application/json")
                .body(json.to_string())
        }
    }
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
                    .route("/tile_range", web::get().to(serve_journey_tile_range))
                    .route(
                        "/tile_range",
                        web::method(Method::OPTIONS).to(handle_preflight),
                    )
                    .route("/random_data", web::get().to(serve_random_data))
                    .route(
                        "/random_data",
                        web::method(Method::OPTIONS).to(handle_preflight),
                    )
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
        let map_renderer = self.map_renderer.lock().unwrap();
        let camera_option =
            get_default_camera_option_from_journey_bitmap(map_renderer.peek_latest_bitmap());

        match camera_option {
            Some(camera) => format!(
                "{}#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&lng={}&lat={}&zoom={}&access_key={}",
                dev_server,
                cgi_host,
                self.port,
                camera.lng,
                camera.lat,
                camera.zoom,
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
        let map_renderer = self.map_renderer.lock().unwrap();
        let camera_option =
            get_default_camera_option_from_journey_bitmap(map_renderer.peek_latest_bitmap());

        match camera_option {
            Some(camera) => format!(
                "file://{}/journey_kernel/index.html#cgi_endpoint=http%3A%2F%2F{}%3A{}&debug=true&lng={}&lat={}&zoom={}&access_key={}", 
                std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string()), 
                cgi_host,
                self.port,
                camera.lng,
                camera.lat,
                camera.zoom,
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
    use memolanes_core::renderer::internal_server::TileRangeQuery;

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

        let json = serde_json::to_string(&original_query).expect("Failed to serialize");
        assert_eq!(
            json,
            r#"{"x":-999,"y":999,"z":20,"width":4096,"height":2048,"buffer_size_power":12,"cached_version":"test-version-123"}"#
        );

        let deserialized: TileRangeQuery =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(original_query.x, deserialized.x);
        assert_eq!(original_query.z, deserialized.z);
        assert_eq!(original_query.buffer_size_power, deserialized.buffer_size_power);
        assert_eq!(original_query.cached_version, deserialized.cached_version);
    }
}
