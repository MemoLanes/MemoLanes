use std::collections::BTreeMap;

use geo_data_format::{Locale, Worldview};
use geo_rasterizer::names::{build_region_names, write_region_names};
use geo_rasterizer::overrides::Overrides;
use geo_rasterizer::parse::ParsedFeature;
use geo_types::{Coord, LineString, MultiPolygon, Polygon};

fn sq() -> MultiPolygon<f64> {
    MultiPolygon(vec![Polygon::new(
        LineString(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]),
        vec![],
    )])
}

/// A single-feature `TYPE == "Country"` group in Asia, with the given zh name.
fn feat(adm0: &str, zh: &str) -> ParsedFeature {
    ParsedFeature {
        adm0_a3: adm0.into(),
        iso_a3: adm0.into(),
        iso_a3_eh: adm0.into(),
        name: adm0.into(),
        feature_type: "Country".into(),
        continent: "Asia".into(),
        region_un: "Asia".into(),
        geometry: sq(),
        localized_names: BTreeMap::from([
            ("NAME_EN".to_string(), adm0.to_string()),
            ("NAME_ZH".to_string(), zh.to_string()),
        ]),
    }
}

// Continents have no NE feature, so `AS` must be authored or generation errors.
const OVERRIDES: &str = "[\"continent.AS\"]\nen-US = \"Asia\"\nzh-CN = \"亚洲\"\n";

#[test]
fn names_resolve_from_ne_fields_and_overrides() {
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国")]),
        (Worldview::Chn, vec![feat("AAA", "甲国")]),
    ];
    let out = build_region_names(&by, &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "甲国");
    assert_eq!(out[&Locale::EnUs]["country.AAA"], "AAA");
    assert_eq!(out[&Locale::ZhCn]["continent.AS"], "亚洲");
}

#[test]
fn diverging_ne_names_across_worldviews_is_an_error() {
    // The shared key holds one name; an uncovered cross-worldview divergence
    // must fail loudly, not silently ship the first worldview's name to all.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国-iso")]),
        (Worldview::Chn, vec![feat("AAA", "甲国-chn")]),
    ];
    let err = build_region_names(&by, &ov).unwrap_err().to_string();
    assert!(err.contains("AAA"), "got: {err}");
    assert!(err.contains("diverge"), "got: {err}");
}

#[test]
fn scoped_overrides_resolve_a_ne_divergence() {
    // The escape hatch the divergence error advertises: scope the divergent
    // worldview, and the remaining unscoped worldviews agree on the shared key.
    let toml = format!("{OVERRIDES}\n[\"country.AAA\".chn]\nzh-CN = \"甲国-chn\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国-iso")]),
        (Worldview::Chn, vec![feat("AAA", "甲国-chn")]),
    ];
    let out = build_region_names(&by, &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "甲国-iso");
    assert_eq!(out[&Locale::ZhCn]["chn.country.AAA"], "甲国-chn");
    // en-US names agree, and the zh-only scope must not leak into en-US.
    assert_eq!(out[&Locale::EnUs]["country.AAA"], "AAA");
    assert!(!out[&Locale::EnUs].contains_key("chn.country.AAA"));
}

#[test]
fn a_worldview_agnostic_override_resolves_a_ne_divergence() {
    let toml = format!("{OVERRIDES}\n[\"country.AAA\"]\nzh-CN = \"甲国\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国-iso")]),
        (Worldview::Chn, vec![feat("AAA", "甲国-chn")]),
    ];
    let out = build_region_names(&by, &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "甲国");
}

#[test]
fn a_dead_override_key_is_an_error() {
    // A typo'd (or removed-entity) override must fail generation, not ship
    // silently as an unused key.
    let toml = format!("{OVERRIDES}\n[\"country.TWM\"]\nzh-CN = \"台湾\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![(Worldview::Iso, vec![feat("AAA", "甲国")])];
    let err = build_region_names(&by, &ov).unwrap_err().to_string();
    assert!(err.contains("country.TWM"), "got: {err}");
    assert!(err.contains("dead override"), "got: {err}");
}

#[test]
fn no_sovereign_and_no_override_is_an_error() {
    // A collapsed multi-member group with no `TYPE == "Country"` member has no
    // NE name to take; without an override the shared key cannot be filled.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let dependency = |name: &str| ParsedFeature {
        feature_type: "Dependency".into(),
        name: name.into(),
        ..feat("BBB", "乙国")
    };
    let by = vec![(Worldview::Iso, vec![dependency("b1"), dependency("b2")])];
    let err = build_region_names(&by, &ov).unwrap_err().to_string();
    assert!(err.contains("country.BBB"), "got: {err}");
    assert!(err.contains("override"), "got: {err}");
}

#[test]
fn region_names_are_written_as_nested_json() {
    let toml = format!("{OVERRIDES}\n[\"country.AAA\".chn]\nzh-CN = \"甲国-chn\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国")]),
        (Worldview::Chn, vec![feat("AAA", "甲国")]),
    ];
    let out = build_region_names(&by, &ov).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = write_region_names(dir.path(), Locale::ZhCn, &out[&Locale::ZhCn]).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    // Same shape as the UI translation files: levels, not dotted keys.
    assert_eq!(json["country"]["AAA"], "甲国");
    assert_eq!(json["continent"]["AS"], "亚洲");
    assert_eq!(json["chn"]["country"]["AAA"], "甲国-chn");
}
