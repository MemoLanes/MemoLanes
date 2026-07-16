use std::collections::BTreeMap;

use geo_data_format::{Locale, Worldview};
use geo_rasterizer::names::build_region_names;
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
const OVERRIDES: &str = "[\"continent.AS.name\"]\nen-US = \"Asia\"\nzh-CN = \"亚洲\"\n";

#[test]
fn names_resolve_from_ne_fields_and_overrides() {
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国")]),
        (Worldview::Chn, vec![feat("AAA", "甲国")]),
    ];
    let out = build_region_names(&by, &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA.name"], "甲国");
    assert_eq!(out[&Locale::EnUs]["country.AAA.name"], "AAA");
    assert_eq!(out[&Locale::ZhCn]["continent.AS.name"], "亚洲");
}

#[test]
fn diverging_ne_names_across_worldviews_is_an_error() {
    // The flat map holds one name per key; a real cross-worldview divergence must
    // fail loudly, not silently ship the first worldview's name to all.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "甲国-iso")]),
        (Worldview::Chn, vec![feat("AAA", "甲国-chn")]),
    ];
    let err = build_region_names(&by, &ov).unwrap_err().to_string();
    assert!(err.contains("AAA"), "got: {err}");
    assert!(err.contains("diverge"), "got: {err}");
}
