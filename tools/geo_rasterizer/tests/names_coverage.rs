//! Coverage gate for the generated region-name maps against the REAL shipped
//! assets: every entity in every worldview's `.bin` must have a non-empty name
//! in every locale's `region_names.<locale>.json` (nested JSON, flattened back
//! to dotted keys here). Without this a region would render as a raw
//! `country.XYZ` key on screen.
//!
//! Assets are gitignored and produced by `just rasterize-geo`, which
//! `just test-geo` runs first. A missing asset is a HARD FAILURE (a gate that
//! skips is a gate that passes).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use geo_data_format::{read_geo_data, Locale, Worldview};
use geo_rasterizer::names::region_names_path;

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/assets/geo")
}

fn require(path: &Path) {
    assert!(
        path.exists(),
        "{} is absent — run `just rasterize-geo` (gitignored build product); this gate checks the \
         SHIPPED assets and would pass vacuously without them",
        path.display()
    );
}

fn entity_keys(worldview: Worldview) -> BTreeSet<String> {
    let path = assets_dir().join(format!("geo_data_{}.bin", worldview.spec().id));
    require(&path);
    read_geo_data(&fs::read(&path).unwrap())
        .unwrap()
        .entities
        .iter()
        .map(|e| e.name_key.clone())
        .collect()
}

fn names(locale: Locale) -> std::collections::BTreeMap<String, String> {
    let path = region_names_path(&assets_dir(), locale);
    require(&path);
    let root: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let mut out = std::collections::BTreeMap::new();
    flatten("", &root, &mut out);
    out
}

fn flatten(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut std::collections::BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        other => panic!("{prefix}: unexpected JSON node {other}"),
    }
}

#[test]
fn every_entity_has_a_name_in_every_locale() {
    for &locale in Locale::ALL {
        let map = names(locale);
        assert!(!map.is_empty(), "{}: empty region names", locale.spec().tag);

        for &worldview in Worldview::ALL {
            let keys = entity_keys(worldview);
            assert!(
                keys.iter().any(|k| k.starts_with("continent.")),
                "{}: bin has no continents — asset looks wrong",
                worldview.spec().id
            );
            assert!(
                keys.iter().any(|k| k.starts_with("province.")),
                "{}: bin has no provinces — asset looks wrong",
                worldview.spec().id
            );

            for key in &keys {
                match map.get(key) {
                    Some(name) => assert!(
                        !name.trim().is_empty(),
                        "{}/{}: empty name for {key}",
                        worldview.spec().id,
                        locale.spec().tag
                    ),
                    None => panic!(
                        "{}/{}: no name for {key} — the app would render a raw key",
                        worldview.spec().id,
                        locale.spec().tag
                    ),
                }
            }
        }
    }
}

/// CLDR must actually be supplying the English province names it is pinned and
/// downloaded for. Nothing else would notice a join-key regression: the chain
/// would fall through to Natural Earth for every one of the 4,587 provinces,
/// each key would still resolve, and every other gate here would still pass
/// while the asset quietly shipped NE's spellings in place of CLDR's.
#[test]
fn cldr_supplies_the_english_province_names_it_is_pinned_for() {
    use geo_rasterizer::admin1::parse_admin1;
    use geo_rasterizer::cldr::load_subdivisions;
    use geo_rasterizer::names::subdivision_key;
    use std::collections::BTreeMap;

    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let admin1_path = crate_dir
        .join("natural_earth")
        .join(geo_data_format::ADMIN1_SOURCE_FILENAME);
    assert!(
        admin1_path.exists(),
        "{} is absent — run `just fetch-natural-earth`",
        admin1_path.display()
    );
    let tag = Locale::EnUs.spec().cldr_tag;
    let subdivisions = load_subdivisions(
        &crate_dir
            .join("cldr")
            .join(format!("subdivisions.{tag}.json")),
        tag,
    )
    .unwrap();

    // The union over worldviews, which is the set `build_region_names` keys on,
    // plus the per-key claim counts its uniqueness rule needs.
    let mut features: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    let mut claims: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for &worldview in Worldview::ALL {
        for f in parse_admin1(&admin1_path, worldview).unwrap() {
            let key = subdivision_key(&f.iso_3166_2);
            claims
                .entry(key.clone())
                .or_default()
                .insert(f.adm1_code.clone());
            features
                .entry(f.adm1_code.clone())
                .or_insert((key, f.name_en));
        }
    }

    let english = names(Locale::EnUs);
    let (mut from_cldr, mut from_ne) = (0usize, 0usize);
    for (adm1_code, (key, name_en)) in &features {
        let shipped = english
            .get(&format!("province.{adm1_code}"))
            .unwrap_or_else(|| panic!("no shipped en-US name for {adm1_code}"));
        // Step 2 of the chain: CLDR, but only where exactly one NE feature
        // claims the key.
        match subdivisions.get(key).filter(|_| claims[key].len() == 1) {
            Some(cldr_name) => {
                assert_eq!(shipped, cldr_name, "{adm1_code} should carry CLDR's name");
                from_cldr += 1;
            }
            None => {
                assert_eq!(
                    Some(shipped.as_str()),
                    name_en.as_deref(),
                    "{adm1_code} should have fallen through to Natural Earth"
                );
                from_ne += 1;
            }
        }
    }

    // Pinned against the pinned sources. A drop here means the ISO 3166-2 join
    // broke, not that the data moved: both sides are hash-verified.
    assert_eq!(from_cldr, 4057, "provinces named by CLDR");
    assert_eq!(from_ne, 530, "provinces that fell through to Natural Earth");
}
