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
