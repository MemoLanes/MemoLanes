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

/// A single-feature `TYPE == "Country"` group in Asia, joined to CLDR by `a2`.
fn feat(adm0: &str, a2: &str) -> ParsedFeature {
    ParsedFeature {
        adm0_a3: adm0.into(),
        iso_a3: adm0.into(),
        iso_a3_eh: adm0.into(),
        iso_a2_eh: a2.into(),
        name: adm0.into(),
        feature_type: "Country".into(),
        continent: "Asia".into(),
        region_un: "Asia".into(),
        geometry: sq(),
    }
}

/// CLDR territories for both locales from `(alpha2, en, zh)` triples.
fn cldr(pairs: &[(&str, &str, &str)]) -> BTreeMap<Locale, BTreeMap<String, String>> {
    let mut en = BTreeMap::new();
    let mut zh = BTreeMap::new();
    for (a2, e, z) in pairs {
        en.insert(a2.to_string(), e.to_string());
        zh.insert(a2.to_string(), z.to_string());
    }
    BTreeMap::from([(Locale::EnUs, en), (Locale::ZhCn, zh)])
}

// Continents have no CLDR territory, so `AS` must be authored or generation errors.
const OVERRIDES: &str = "[\"continent.AS\"]\nen-US = \"Asia\"\nzh-CN = \"亚洲\"\n";

#[test]
fn names_resolve_from_cldr_and_overrides() {
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "AA")]),
        (Worldview::Chn, vec![feat("AAA", "AA")]),
    ];
    let out = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "甲国");
    assert_eq!(out[&Locale::EnUs]["country.AAA"], "Aaa");
    assert_eq!(out[&Locale::ZhCn]["continent.AS"], "亚洲");
}

#[test]
fn diverging_iso_a2_across_worldviews_is_an_error() {
    // A code must denote one territory: the same ADM0_A3 mapping to different
    // ISO_A2_EH across worldviews is a data fault, not a name to pick from.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "AA")]),
        (Worldview::Chn, vec![feat("AAA", "AB")]),
    ];
    let err = build_region_names(
        &by,
        &cldr(&[("AA", "Aaa", "甲国"), ("AB", "Abb", "乙国")]),
        &ov,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("AAA"), "got: {err}");
    assert!(err.contains("ISO_A2_EH"), "got: {err}");
}

#[test]
fn a_cldr_miss_without_override_is_an_error() {
    // The join key resolves but CLDR carries no such territory (an NE-only
    // aggregate) — must fail loudly, not ship a raw key.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let by = vec![(Worldview::Iso, vec![feat("AAA", "ZZ")])];
    let err = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov)
        .unwrap_err()
        .to_string();
    assert!(err.contains("country.AAA"), "got: {err}");
    assert!(err.contains("ZZ"), "got: {err}");
}

#[test]
fn a_worldview_agnostic_override_beats_cldr() {
    let toml = format!("{OVERRIDES}\n[\"country.AAA\"]\nzh-CN = \"乙国\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![(Worldview::Iso, vec![feat("AAA", "AA")])];
    let out = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov).unwrap();
    // zh override wins; en falls through to CLDR (no en override, no leak).
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "乙国");
    assert_eq!(out[&Locale::EnUs]["country.AAA"], "Aaa");
}

#[test]
fn a_scoped_override_emits_a_prefixed_key() {
    // The worldview-scoped override path (future admin-1): a `<worldview>.<key>`
    // key the app prefers, without disturbing the CLDR-resolved shared key.
    let toml = format!("{OVERRIDES}\n[\"country.AAA\".chn]\nzh-CN = \"甲国-chn\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "AA")]),
        (Worldview::Chn, vec![feat("AAA", "AA")]),
    ];
    let out = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov).unwrap();
    assert_eq!(out[&Locale::ZhCn]["country.AAA"], "甲国");
    assert_eq!(out[&Locale::ZhCn]["chn.country.AAA"], "甲国-chn");
    // The zh-only scope must not leak into en-US.
    assert_eq!(out[&Locale::EnUs]["country.AAA"], "Aaa");
    assert!(!out[&Locale::EnUs].contains_key("chn.country.AAA"));
}

#[test]
fn a_dead_override_key_is_an_error() {
    // A typo'd (or removed-entity) override must fail generation, not ship
    // silently as an unused key.
    let toml = format!("{OVERRIDES}\n[\"country.TWM\"]\nzh-CN = \"台湾\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![(Worldview::Iso, vec![feat("AAA", "AA")])];
    let err = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov)
        .unwrap_err()
        .to_string();
    assert!(err.contains("country.TWM"), "got: {err}");
    assert!(err.contains("dead override"), "got: {err}");
}

#[test]
fn no_sovereign_and_no_override_is_an_error() {
    // A collapsed multi-member group with no `TYPE == "Country"` member has no
    // sovereign ISO_A2_EH to join on; without an override the key cannot fill.
    let ov = Overrides::from_toml_str(OVERRIDES).unwrap();
    let dependency = |name: &str| ParsedFeature {
        feature_type: "Dependency".into(),
        name: name.into(),
        ..feat("BBB", "BB")
    };
    let by = vec![(Worldview::Iso, vec![dependency("b1"), dependency("b2")])];
    let err = build_region_names(&by, &cldr(&[("BB", "Bbb", "乙国")]), &ov)
        .unwrap_err()
        .to_string();
    assert!(err.contains("country.BBB"), "got: {err}");
    assert!(err.contains("override"), "got: {err}");
}

#[test]
fn region_names_are_written_as_nested_json() {
    let toml = format!("{OVERRIDES}\n[\"country.AAA\".chn]\nzh-CN = \"甲国-chn\"\n");
    let ov = Overrides::from_toml_str(&toml).unwrap();
    let by = vec![
        (Worldview::Iso, vec![feat("AAA", "AA")]),
        (Worldview::Chn, vec![feat("AAA", "AA")]),
    ];
    let out = build_region_names(&by, &cldr(&[("AA", "Aaa", "甲国")]), &ov).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = write_region_names(dir.path(), Locale::ZhCn, &out[&Locale::ZhCn]).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    // Same shape as the UI translation files: levels, not dotted keys.
    assert_eq!(json["country"]["AAA"], "甲国");
    assert_eq!(json["continent"]["AS"], "亚洲");
    assert_eq!(json["chn"]["country"]["AAA"], "甲国-chn");
}
