use std::path::Path;

use geo_rasterizer::parse::{parse_geojson, validate_no_antimeridian_span};

#[test]
fn parse_geojson_drops_no_features_in_synthetic() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    assert_eq!(features.len(), 3);
    let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
    assert!(codes.contains(&"AAA"));
    assert!(codes.contains(&"BBB"));
    assert!(codes.contains(&"CCC"));
}

#[test]
fn parse_geojson_keeps_seven_seas() {
    let raw = std::fs::read_to_string("tests/fixtures/synthetic.geojson").unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let features = value["features"].as_array_mut().unwrap();
    features.push(serde_json::json!({
        "type": "Feature",
        "properties": {
            "ADM0_A3": "OCN",
            "ISO_A3": "-99",
            "ISO_A3_EH": "OCN",
            "NAME": "Open Ocean",
            "CONTINENT": "Seven seas (open ocean)",
            "REGION_UN": "Americas",
            "TYPE": "Sovereign country"
        },
        "geometry": {
            "type": "Polygon",
            "coordinates": [[[100.0, 0.0], [101.0, 0.0], [101.0, 1.0], [100.0, 1.0], [100.0, 0.0]]]
        }
    }));
    let augmented = serde_json::to_string(&value).unwrap();
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), augmented).unwrap();
    let features = parse_geojson(tmp.path(), "iso").unwrap();
    assert_eq!(features.len(), 4); // OCN kept, not dropped
    let ocn = features.iter().find(|f| f.adm0_a3 == "OCN").unwrap();
    // Its continent parent is resolved later from REGION_UN.
    assert_eq!(ocn.region_un, "Americas");
}

#[test]
fn antimeridian_validation_passes_on_synthetic() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    validate_no_antimeridian_span(&features).unwrap();
}

#[test]
fn antimeridian_validation_rejects_overspanning_ring() {
    let raw = serde_json::json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {
                "ADM0_A3": "BAD",
                "ISO_A3": "BAD",
                "NAME": "Bad",
                "CONTINENT": "Asia",
                "REGION_UN": "Asia",
                "TYPE": "Sovereign country"
            },
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[-170.0, 0.0], [170.0, 0.0], [170.0, 1.0], [-170.0, 1.0], [-170.0, 0.0]]]
            }
        }]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_geojson(tmp.path(), "iso").unwrap();
    let err = validate_no_antimeridian_span(&features).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("antimeridian"),
        "got: {err}"
    );
}

#[test]
fn antimeridian_validation_allows_polar_cap() {
    // Antarctica-like polygon: ring entirely below -85° latitude.
    let raw = serde_json::json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {
                "ADM0_A3": "ATA",
                "ISO_A3": "ATA",
                "NAME": "Antarctica",
                "CONTINENT": "Antarctica",
                "REGION_UN": "Antarctica",
                "TYPE": "Indeterminate"
            },
            "geometry": {
                "type": "Polygon",
                "coordinates": [[
                    [-180.0, -89.0],
                    [180.0, -89.0],
                    [180.0, -86.0],
                    [-180.0, -86.0],
                    [-180.0, -89.0]
                ]]
            }
        }]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_geojson(tmp.path(), "iso").unwrap();
    // All 360° edges connect vertices at |lat| > 85° → allowed.
    validate_no_antimeridian_span(&features).expect("polar cap should pass");
}
