//! Absorptions are applied inside `parse_admin0`, so they are tested through
//! that public door (the function itself is crate-private by design).

use geo_rasterizer::admin0::parse_admin0;
use serde_json::json;

/// Write a chn-style FeatureCollection with one square per `(code, type)` pair
/// (each offset so a parent's merged geometry has a distinct polygon per part)
/// and return the temp file holding it.
fn write_source(features: &[(&str, &str)]) -> tempfile::NamedTempFile {
    let features: Vec<_> = features
        .iter()
        .enumerate()
        .map(|(i, (code, feature_type))| {
            let x0 = i as f64 * 10.0;
            json!({
                "type": "Feature",
                "properties": {"ADM0_A3":code,"ISO_A3":code,"ISO_A3_EH":code,"NAME":code,"CONTINENT":"Asia","REGION_UN":"Asia","TYPE":feature_type},
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
fn a_demoted_feature_survives_tagged_while_its_parent_gains_its_geometry() {
    let src = write_source(&[("CHN", "Country"), ("HKG", "Country"), ("JPN", "Country")]);
    let features = parse_admin0(src.path(), "chn").unwrap();
    let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
    assert_eq!(codes, vec!["CHN", "HKG", "JPN"]);
    let china = features.iter().find(|f| f.adm0_a3 == "CHN").unwrap();
    assert_eq!(china.geometry.0.len(), 2, "China still gains HK's polygon");
    let hkg = features.iter().find(|f| f.adm0_a3 == "HKG").unwrap();
    assert_eq!(hkg.demoted_into.as_deref(), Some("CHN"));
    assert_eq!(hkg.geometry.0.len(), 1, "HKG keeps its own geometry too");
}

#[test]
fn a_merged_feature_is_removed_as_before() {
    let src = write_source(&[("CHN", "Country"), ("SCR", "Indeterminate")]);
    let features = parse_admin0(src.path(), "chn").unwrap();
    let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
    assert_eq!(codes, vec!["CHN"]);
    assert_eq!(features[0].geometry.0.len(), 2);
}

#[test]
fn a_merge_of_a_country_typed_feature_is_an_error() {
    let src = write_source(&[("CHN", "Country"), ("SCR", "Country")]);
    let err = parse_admin0(src.path(), "chn").err().unwrap().to_string();
    assert!(err.contains("SCR") && err.contains("Country"), "got: {err}");
}

#[test]
fn other_worldviews_are_untouched() {
    for wv in ["iso", "usa"] {
        let src = write_source(&[("CHN", "Country"), ("HKG", "Country"), ("MAC", "Country")]);
        let features = parse_admin0(src.path(), wv).unwrap();
        let codes: Vec<&str> = features.iter().map(|f| f.adm0_a3.as_str()).collect();
        assert_eq!(codes, vec!["CHN", "HKG", "MAC"], "worldview {wv}");
        assert!(features.iter().all(|f| f.demoted_into.is_none()));
    }
}

#[test]
fn missing_absorption_target_is_an_error() {
    // HKG present, but its target CHN is not — the geometry merge would lose land.
    let src = write_source(&[("HKG", "Country"), ("JPN", "Country")]);
    // `.err()` avoids requiring `Admin0Feature: Debug` (which `unwrap_err` needs).
    let err = parse_admin0(src.path(), "chn").err().unwrap().to_string();
    assert!(err.contains("CHN"), "got: {err}");
}
