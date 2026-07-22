//! Absorptions are applied inside `parse_geojson`, so they are tested through
//! that public door (the function itself is crate-private by design).

use geo_rasterizer::parse::parse_geojson;
use serde_json::json;

/// Write a chn-style FeatureCollection with one `TYPE == "Country"` square per
/// code (each offset so China's merged geometry has a distinct polygon per part)
/// and return the temp file holding it.
fn write_source(codes: &[&str]) -> tempfile::NamedTempFile {
    let features: Vec<_> = codes
        .iter()
        .enumerate()
        .map(|(i, code)| {
            let x0 = i as f64 * 10.0;
            json!({
                "type": "Feature",
                "properties": {"ADM0_A3":code,"ISO_A3":code,"ISO_A3_EH":code,"NAME":code,"CONTINENT":"Asia","REGION_UN":"Asia","TYPE":"Country"},
                "geometry": {"type":"Polygon","coordinates":[[[x0,0.0],[x0+1.0,0.0],[x0+1.0,1.0],[x0,0.0]]]}
            })
        })
        .collect();
    let raw = json!({"type": "FeatureCollection", "features": features});
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    tmp
}

#[test]
fn chn_absorbs_hong_kong_and_macau_into_china() {
    let src = write_source(&["CHN", "HKG", "MAC", "JPN"]);
    let features = parse_geojson(src.path(), "chn").unwrap();
    let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
    assert_eq!(codes, vec!["CHN", "JPN"]);
    let china = features.iter().find(|f| f.adm0_a3 == "CHN").unwrap();
    assert_eq!(china.geometry.0.len(), 3, "China gains HK + Macau polygons");
}

#[test]
fn other_worldviews_keep_hong_kong_and_macau_separate() {
    for wv in ["iso", "usa"] {
        let src = write_source(&["CHN", "HKG", "MAC"]);
        let features = parse_geojson(src.path(), wv).unwrap();
        let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
        assert_eq!(codes, vec!["CHN", "HKG", "MAC"], "worldview {wv}");
    }
}

#[test]
fn missing_sovereign_is_an_error() {
    // HKG present, but its sovereign CHN is not — its geometry would vanish.
    let src = write_source(&["HKG", "JPN"]);
    // `.err()` avoids requiring `ParsedFeature: Debug` (which `unwrap_err` needs).
    let err = parse_geojson(src.path(), "chn").err().unwrap().to_string();
    assert!(err.contains("CHN"), "got: {err}");
}
