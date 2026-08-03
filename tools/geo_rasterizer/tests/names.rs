use std::collections::BTreeMap;

use geo_data_format::{Locale, Worldview};
use geo_rasterizer::admin0::Admin0Feature;
use geo_rasterizer::admin1::Admin1Feature;
use geo_rasterizer::names::{build_region_names, write_region_names};
use geo_rasterizer::overrides::Overrides;
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
fn feat(adm0: &str, a2: &str) -> Admin0Feature {
    Admin0Feature {
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
    let out = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
    .unwrap();
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
        &[],
        &cldr(&[("AA", "Aaa", "甲国"), ("AB", "Abb", "乙国")]),
        &BTreeMap::new(),
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
    let err = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
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
    let out = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
    .unwrap();
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
    let out = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
    .unwrap();
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
    let err = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
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
    let dependency = |name: &str| Admin0Feature {
        feature_type: "Dependency".into(),
        name: name.into(),
        ..feat("BBB", "BB")
    };
    let by = vec![(Worldview::Iso, vec![dependency("b1"), dependency("b2")])];
    let err = build_region_names(
        &by,
        &[],
        &cldr(&[("BB", "Bbb", "乙国")]),
        &BTreeMap::new(),
        &ov,
    )
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
    let out = build_region_names(
        &by,
        &[],
        &cldr(&[("AA", "Aaa", "甲国")]),
        &BTreeMap::new(),
        &ov,
    )
    .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = write_region_names(dir.path(), Locale::ZhCn, &out[&Locale::ZhCn]).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    // Same shape as the UI translation files: levels, not dotted keys.
    assert_eq!(json["country"]["AAA"], "甲国");
    assert_eq!(json["continent"]["AS"], "亚洲");
    assert_eq!(json["chn"]["country"]["AAA"], "甲国-chn");
}

fn admin1(code: &str, iso: &str, en: &str, zh: &str) -> Admin1Feature {
    Admin1Feature {
        adm1_code: code.into(),
        adm0_a3: "XXX".into(),
        iso_3166_2: iso.into(),
        name_en: Some(en.into()),
        name_zh: Some(zh.into()),
        geometry: geo_types::MultiPolygon(vec![]),
    }
}

/// Run the real name builder over one worldview with no admin-0 features, so a
/// test asserts only about provinces. Takes the features by value — do NOT add
/// a `Clone` derive to `Admin1Feature` to avoid this; production code has
/// no need to clone one, and widening a type for a test's convenience is not
/// something this codebase does.
fn build_names_for_test(
    provinces: Vec<Admin1Feature>,
    cldr_subdivisions: BTreeMap<Locale, BTreeMap<String, String>>,
) -> BTreeMap<Locale, BTreeMap<String, String>> {
    build_region_names(
        &[(Worldview::Iso, vec![])],
        &[(Worldview::Iso, provinces)],
        &empty_cldr_territories(),
        &cldr_subdivisions,
        &Overrides::default(),
    )
    .expect("names must build")
}

fn build_names_for_test_err(
    provinces: Vec<Admin1Feature>,
    cldr_subdivisions: BTreeMap<Locale, BTreeMap<String, String>>,
) -> String {
    build_region_names(
        &[(Worldview::Iso, vec![])],
        &[(Worldview::Iso, provinces)],
        &empty_cldr_territories(),
        &cldr_subdivisions,
        &Overrides::default(),
    )
    .unwrap_err()
    .to_string()
}

fn empty_cldr_territories() -> BTreeMap<Locale, BTreeMap<String, String>> {
    Locale::ALL.iter().map(|&l| (l, BTreeMap::new())).collect()
}

/// Like `build_names_for_test`, but for cases that need admin-1 features to
/// differ across worldviews — `build_names_for_test` hardcodes a single
/// `Worldview::Iso` entry and cannot express that.
fn build_names_for_test_by_worldview(
    admin1_by_worldview: Vec<(Worldview, Vec<Admin1Feature>)>,
    cldr_subdivisions: BTreeMap<Locale, BTreeMap<String, String>>,
) -> BTreeMap<Locale, BTreeMap<String, String>> {
    build_region_names(
        &[(Worldview::Iso, vec![])],
        &admin1_by_worldview,
        &empty_cldr_territories(),
        &cldr_subdivisions,
        &Overrides::default(),
    )
    .expect("names must build")
}

#[test]
fn subdivision_key_matches_the_cldr_bcp47_form() {
    use geo_rasterizer::names::subdivision_key;
    assert_eq!(subdivision_key("US-CA"), "usca");
    assert_eq!(subdivision_key("JP-01"), "jp01");
    assert_eq!(subdivision_key("GB-DRY"), "gbdry");
}

#[test]
fn cldr_wins_for_a_unique_key_and_ne_wins_for_a_shared_one() {
    // Two features carrying the SAME iso_3166_2 — CLDR names exactly one
    // territory per key, so neither may take the CLDR name.
    let shared_a = admin1("XX-001", "SD-DS", "Southern Darfur", "南达尔富尔");
    let shared_b = admin1("XX-002", "SD-DS", "Eastern Darfur", "东达尔富尔");
    let unique = admin1("XX-003", "US-CA", "California NE", "加州NE");

    let cldr_sub = BTreeMap::from([
        ("sdds".to_string(), "South Darfur".to_string()),
        ("usca".to_string(), "California".to_string()),
    ]);
    let names = build_names_for_test(
        vec![shared_a, shared_b, unique],
        BTreeMap::from([(Locale::EnUs, cldr_sub)]),
    );

    let en = &names[&Locale::EnUs];
    assert_eq!(en["province.XX-003"], "California");
    assert_eq!(en["province.XX-001"], "Southern Darfur");
    assert_eq!(en["province.XX-002"], "Eastern Darfur");
}

#[test]
fn a_locale_without_a_subdivisions_pin_uses_ne_throughout() {
    let unique = admin1("XX-003", "US-CA", "California NE", "加利福尼亚");
    // zh-CN has no CLDR subdivisions map at all.
    let names = build_names_for_test(vec![unique], BTreeMap::new());
    assert_eq!(names[&Locale::ZhCn]["province.XX-003"], "加利福尼亚");
}

#[test]
fn a_province_with_no_name_anywhere_is_a_hard_error() {
    let nameless = Admin1Feature {
        adm1_code: "XX-004".into(),
        adm0_a3: "XXX".into(),
        iso_3166_2: "XX-X01~".into(),
        name_en: None,
        name_zh: None,
        geometry: geo_types::MultiPolygon(vec![]),
    };
    let err = build_names_for_test_err(vec![nameless], BTreeMap::new());
    assert!(err.contains("province.XX-004"), "got: {err}");
}

#[test]
fn ambiguity_is_judged_over_the_union_of_worldviews_not_per_worldview() {
    // SD-DS is claimed by two features in `iso`, but NE's per-worldview
    // FCLASS_* filter (see `admin1::parse_admin1`) can legitimately drop one
    // of them in another worldview, leaving only one claimant there. A
    // per-worldview uniqueness check would then see `chn` alone and call the
    // key unique — the survivor must still keep its NE name, because the key
    // is genuinely ambiguous once every worldview is considered together.
    let shared_a_iso = admin1("XX-001", "SD-DS", "Southern Darfur", "南达尔富尔");
    let shared_b_iso = admin1("XX-002", "SD-DS", "Eastern Darfur", "东达尔富尔");
    let shared_a_chn = admin1("XX-001", "SD-DS", "Southern Darfur", "南达尔富尔");

    let cldr_sub = BTreeMap::from([("sdds".to_string(), "South Darfur".to_string())]);
    let names = build_names_for_test_by_worldview(
        vec![
            (Worldview::Iso, vec![shared_a_iso, shared_b_iso]),
            (Worldview::Chn, vec![shared_a_chn]),
        ],
        BTreeMap::from([(Locale::EnUs, cldr_sub)]),
    );

    assert_eq!(names[&Locale::EnUs]["province.XX-001"], "Southern Darfur");
}
