use std::collections::HashMap;

use super::MapRenderer;

use rand::Rng;

fn generate_random_data(size: u64) -> Result<Vec<u8>, String> {
    let max_size = 10_485_760; // 10MB limit

    if size > max_size {
        return Err(format!(
            "Size too large. Maximum allowed: {max_size} bytes (10MB)"
        ));
    }

    let mut data = vec![0u8; size as usize];
    rand::rng().fill_bytes(&mut data);

    Ok(data)
}

/// Unified response for all webview requests.
pub struct WebviewResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
}

/// Unified request dispatcher. Routes path to the appropriate handler,
/// parses query params, and returns a fully-formed response.
/// Always returns status 200 or 500 -- "not modified" is signaled via
/// X-Not-Modified header (Android WebResourceResponse rejects 3xx codes).
pub fn dispatch_request(
    path: &str,
    query_params: &HashMap<String, String>,
    map_renderer: &mut MapRenderer,
) -> WebviewResponse {
    match path {
        "tile_range" => dispatch_tile_range(query_params, map_renderer),
        "random_data" => dispatch_random_data(query_params),
        _ => WebviewResponse {
            status: 500,
            content_type: "text/plain".to_string(),
            body: format!("Unknown route: {path}").into_bytes(),
            headers: HashMap::new(),
        },
    }
}

fn parse_or<T: std::str::FromStr>(params: &HashMap<String, String>, key: &str, default: T) -> T {
    params
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn dispatch_tile_range(
    params: &HashMap<String, String>,
    map_renderer: &mut MapRenderer,
) -> WebviewResponse {
    if map_renderer.matches_version(params.get("cached_version").map(String::as_str)) {
        return WebviewResponse {
            status: 200,
            content_type: "application/octet-stream".to_string(),
            body: Vec::new(),
            headers: HashMap::from([("X-Not-Modified".to_string(), "true".to_string())]),
        };
    }

    match map_renderer.get_tile_range_response(
        parse_or(params, "x", 0),
        parse_or(params, "y", 0),
        parse_or(params, "z", 0),
        parse_or(params, "width", 1),
        parse_or(params, "height", 1),
        parse_or(params, "buffer_size_power", 8),
    ) {
        Ok(body) => WebviewResponse {
            status: 200,
            content_type: "application/octet-stream".to_string(),
            body,
            headers: HashMap::from([(
                "X-Tile-Version".to_string(),
                map_renderer.get_version_string(),
            )]),
        },
        Err(e) => WebviewResponse {
            status: 500,
            content_type: "text/plain".to_string(),
            body: format!("Failed to generate tile buffer: {e}").into_bytes(),
            headers: HashMap::new(),
        },
    }
}

fn dispatch_random_data(params: &HashMap<String, String>) -> WebviewResponse {
    let size: u64 = parse_or(params, "size", 1_048_576);
    match generate_random_data(size) {
        Ok(data) => WebviewResponse {
            status: 200,
            content_type: "application/octet-stream".to_string(),
            body: data,
            headers: HashMap::new(),
        },
        Err(e) => WebviewResponse {
            status: 500,
            content_type: "text/plain".to_string(),
            body: e.into_bytes(),
            headers: HashMap::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journey_bitmap::JourneyBitmap;

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
        assert!(resp.headers.contains_key("X-Tile-Version"));
    }

    #[test]
    fn test_dispatch_tile_range_not_modified() {
        let jb = JourneyBitmap::new();
        let mut mr = MapRenderer::new(jb);
        let version = mr.get_version_string();

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
        .chain(std::iter::once(("cached_version".to_string(), version)))
        .collect();

        let resp = dispatch_request("tile_range", &params, &mut mr);
        assert_eq!(resp.status, 200);
        assert!(resp.body.is_empty());
        assert_eq!(
            resp.headers.get("X-Not-Modified"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn tile_range_version_negotiation_preserves_quoted_and_stale_versions() {
        let mut renderer = MapRenderer::new(JourneyBitmap::new());
        let mut params = HashMap::from([
            ("z".to_string(), "9".to_string()),
            ("buffer_size_power".to_string(), "6".to_string()),
            ("cached_version".to_string(), "\"0\"".to_string()),
        ]);
        let unchanged = dispatch_request("tile_range", &params, &mut renderer);
        assert_eq!(unchanged.status, 200);
        assert!(unchanged.body.is_empty());
        assert_eq!(unchanged.headers.len(), 1);
        assert_eq!(unchanged.headers["X-Not-Modified"], "true");

        renderer.replace(JourneyBitmap::new());
        for version in ["\"0\"", "invalid"] {
            params.insert("cached_version".to_string(), version.to_string());
            let changed = dispatch_request("tile_range", &params, &mut renderer);
            assert_eq!(changed.status, 200);
            assert!(!changed.body.is_empty());
            assert_eq!(changed.headers.len(), 1);
            assert_eq!(
                changed.headers["X-Tile-Version"],
                renderer.get_version_string()
            );
        }

        params.insert("width".to_string(), "0".to_string());
        let invalid = dispatch_request("tile_range", &params, &mut renderer);
        assert_eq!(invalid.status, 500);
        assert_eq!(invalid.content_type, "text/plain");
        assert!(invalid.headers.is_empty());
        assert_eq!(
            String::from_utf8(invalid.body).unwrap(),
            "Failed to generate tile buffer: Invalid dimensions: width=0, height=1"
        );
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
