use std::path::Path;

use geo_data_format::GeoEntityKind;
use geo_rasterizer::entities::{assemble_entities, EntityModel};
use geo_rasterizer::parse::parse_geojson;
use geo_rasterizer::registry::Registry;

const SYNTHETIC_REGISTRY: &str = "tests/fixtures/synthetic_registry.toml";

#[test]
fn assemble_groups_continents_and_countries() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson")).unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model: EntityModel = assemble_entities(&features, &registry).unwrap();

    // 3 distinct continents in synthetic: Europe, Asia, Africa.
    let continent_count = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Continent))
        .count();
    assert_eq!(continent_count, 3);

    // 3 country entities, one per ADM0_A3.
    let country_count = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .count();
    assert_eq!(country_count, 3);
}

#[test]
fn assemble_collapses_duplicate_adm0_a3() {
    use geo_rasterizer::registry::Entry;
    use serde_json::json;
    let raw = json!({
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"FRA","ISO_A3":"-99","NAME":"France","CONTINENT":"Europe","TYPE":"Sovereign country"},
                "geometry": {"type":"Polygon","coordinates":[[[2.0,48.0],[3.0,48.0],[3.0,49.0],[2.0,49.0],[2.0,48.0]]]}
            },
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"FRA","ISO_A3":"GUF","NAME":"French Guiana","CONTINENT":"South America","TYPE":"Country"},
                "geometry": {"type":"Polygon","coordinates":[[[-53.0,4.0],[-52.0,4.0],[-52.0,5.0],[-53.0,5.0],[-53.0,4.0]]]}
            }
        ]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_geojson(tmp.path()).unwrap();
    // Build an inline registry: EU=0, SA=1, FRA=2.
    let registry = Registry {
        schema: 1,
        continents: vec![
            Entry {
                code: "EU".into(),
                id: 0,
                refs: std::collections::BTreeMap::from([("iso".to_string(), [2.5_f64, 48.5_f64])]),
            },
            Entry {
                code: "SA".into(),
                id: 1,
                refs: std::collections::BTreeMap::from([("iso".to_string(), [-52.5_f64, 4.5_f64])]),
            },
        ],
        countries: vec![Entry {
            code: "FRA".into(),
            id: 2,
            refs: std::collections::BTreeMap::from([("iso".to_string(), [2.5_f64, 48.5_f64])]),
        }],
    };
    let model = assemble_entities(&features, &registry).unwrap();
    let fra = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .find(|e| e.iso_code == "FRA")
        .expect("FRA should exist exactly once");
    // Its parent should be Europe (the metropole continent), not South America.
    let parent = model
        .entities
        .iter()
        .find(|e| Some(e.id) == fra.parent_id)
        .unwrap();
    assert_eq!(parent.iso_code, "EU");
    // The collapsed FRA must own both polygons.
    let merged = model.geometry_for_country.get("FRA").unwrap();
    assert_eq!(merged.0.len(), 2);
}

#[test]
fn entity_ids_are_dense_and_continents_first() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson")).unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model = assemble_entities(&features, &registry).unwrap();
    // Continents at IDs 0..continent_count; countries follow (registry assigns 0-2 to
    // continents and 3-5 to countries).
    let mut ids: Vec<u32> = model.entities.iter().map(|e| e.id.0).collect();
    ids.sort();
    assert_eq!(ids, (0..ids.len() as u32).collect::<Vec<_>>());
    let last_continent_id = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Continent))
        .map(|e| e.id.0)
        .max()
        .unwrap();
    let first_country_id = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .map(|e| e.id.0)
        .min()
        .unwrap();
    assert!(first_country_id > last_continent_id);
}

#[test]
fn unused_lookup_value_is_referenced() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson")).unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model = assemble_entities(&features, &registry).unwrap();
    let _value = model.entities[0].id;
}
