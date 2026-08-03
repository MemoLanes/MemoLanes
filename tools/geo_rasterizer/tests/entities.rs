use std::path::Path;

use geo_data_format::{GeoEntityId, GeoEntityKind, Worldview};
use geo_rasterizer::admin0::{parse_admin0, Admin0Feature};
use geo_rasterizer::admin1::Admin1Feature;
use geo_rasterizer::entities::{
    assemble_entities, attach_province_entities, collect_provinces, resolve_province_parents,
    EntityModel,
};
use geo_rasterizer::refine::Coverage;
use geo_rasterizer::registry::{Entry, Registry};
use geo_types::{Coord, LineString, MultiPolygon, Polygon};

const SYNTHETIC_REGISTRY: &str = "tests/fixtures/synthetic_registry.toml";

/// A minimal `TYPE == "Country"` feature: one unit-square polygon, `iso_a3_eh`
/// equal to its `adm0_a3`. `region_un` is a harmless placeholder (unused unless
/// `continent` is the "Seven seas" bucket).
fn feat(adm0: &str, continent: &str) -> Admin0Feature {
    let sq = Polygon::new(
        LineString(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]),
        vec![],
    );
    Admin0Feature {
        adm0_a3: adm0.into(),
        iso_a3: adm0.into(),
        iso_a3_eh: adm0.into(),
        iso_a2_eh: adm0.into(),
        name: adm0.into(),
        feature_type: "Country".into(),
        continent: continent.into(),
        region_un: "Africa".into(),
        geometry: MultiPolygon(vec![sq]),
    }
}

/// Tiny registry: continent `AS` = 5, country `AAA` = 3.
fn reg() -> Registry {
    Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 5,
            point: None,
        }],
        countries: vec![Entry {
            code: "AAA".into(),
            id: 3,
            point: None,
        }],
        provinces: vec![],
    }
}

#[test]
fn ids_come_from_registry_not_position() {
    let m = assemble_entities(&[feat("AAA", "Asia")], &reg()).unwrap();
    let aaa = m
        .entities
        .iter()
        .find(|e| e.canonical_code == "AAA")
        .unwrap();
    assert_eq!(aaa.id, GeoEntityId(3));
    assert_eq!(aaa.parent_id, Some(GeoEntityId(5)));
}

/// End-to-end for the chn worldview: parsing the chn source folds Hong Kong and
/// Macau into China — one CHN entity (no HKG/MAC), its geometry carries all three
/// parts, and its ISO code is the sovereign's (CHN). Taiwan is already merged in
/// NE's chn source, so this covers the whole "HK/Macau/Taiwan → China" grouping
/// the app requires. Drives `parse_admin0` (which applies the absorptions) so it
/// exercises the real, un-forgettable path.
#[test]
fn chn_worldview_merges_hong_kong_macau_and_taiwan_into_china() {
    use serde_json::json;
    // A chn-style source: China plus the two still-distinct dependencies, each a
    // separate square (NE's chn source has no separate Taiwan feature).
    let feature = |adm0: &str, x0: f64| {
        json!({
            "type": "Feature",
            "properties": {"ADM0_A3":adm0,"ISO_A3":adm0,"ISO_A3_EH":adm0,"NAME":adm0,"CONTINENT":"Asia","REGION_UN":"Asia","TYPE":"Country"},
            "geometry": {"type":"Polygon","coordinates":[[[x0,0.0],[x0+1.0,0.0],[x0+1.0,1.0],[x0,0.0]]]}
        })
    };
    let raw = json!({
        "type": "FeatureCollection",
        "features": [feature("CHN", 0.0), feature("HKG", 10.0), feature("MAC", 20.0)],
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_admin0(tmp.path(), "chn").unwrap();

    let reg = Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 0,
            point: None,
        }],
        countries: vec![Entry {
            code: "CHN".into(),
            id: 18,
            point: None,
        }],
        provinces: vec![],
    };
    let m = assemble_entities(&features, &reg).unwrap();

    let countries: Vec<&str> = m
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .map(|e| e.canonical_code.as_str())
        .collect();
    assert_eq!(
        countries,
        vec!["CHN"],
        "HKG/MAC must not survive as entities"
    );

    let china = m
        .entities
        .iter()
        .find(|e| e.canonical_code == "CHN")
        .unwrap();
    assert_eq!(china.iso_a3_eh.as_deref(), Some("CHN"));
    assert_eq!(china.parent_id, Some(GeoEntityId(0)));

    // All three source polygons are merged under CHN; no HKG/MAC geometry.
    assert_eq!(m.geometry_for_country["CHN"].0.len(), 3);
    assert!(!m.geometry_for_country.contains_key("HKG"));
    assert!(!m.geometry_for_country.contains_key("MAC"));
}

#[test]
fn unknown_adm0_is_an_error() {
    let err = assemble_entities(&[feat("ZZZ", "Asia")], &reg())
        .unwrap_err()
        .to_string();
    assert!(err.contains("ZZZ"), "got: {err}");
}

#[test]
fn assemble_groups_continents_and_countries() {
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
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
    use serde_json::json;
    let raw = json!({
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"FRA","ISO_A3":"-99","ISO_A3_EH":"FRA","NAME":"France","CONTINENT":"Europe","REGION_UN":"Europe","TYPE":"Country"},
                "geometry": {"type":"Polygon","coordinates":[[[2.0,48.0],[3.0,48.0],[3.0,49.0],[2.0,49.0],[2.0,48.0]]]}
            },
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"FRA","ISO_A3":"GUF","ISO_A3_EH":"GUF","NAME":"French Guiana","CONTINENT":"South America","REGION_UN":"Americas","TYPE":"Geo unit"},
                "geometry": {"type":"Polygon","coordinates":[[[-53.0,4.0],[-52.0,4.0],[-52.0,5.0],[-53.0,5.0],[-53.0,4.0]]]}
            }
        ]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_admin0(tmp.path(), "iso").unwrap();
    // Build an inline registry: EU=0, SA=1, FRA=2.
    let registry = Registry {
        schema: 1,
        continents: vec![
            Entry {
                code: "EU".into(),
                id: 0,
                point: None,
            },
            Entry {
                code: "SA".into(),
                id: 1,
                point: None,
            },
        ],
        countries: vec![Entry {
            code: "FRA".into(),
            id: 2,
            point: None,
        }],
        provinces: vec![],
    };
    let model = assemble_entities(&features, &registry).unwrap();
    let fra = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .find(|e| e.canonical_code == "FRA")
        .expect("FRA should exist exactly once");
    // Collapsed group: the sovereign is the sole TYPE=="Country" member
    // (France → FRA), not the detached dependency (French Guiana, "Geo unit" → GUF).
    assert_eq!(fra.iso_a3_eh.as_deref(), Some("FRA"));
    // Its parent should be Europe (the metropole continent), not South America.
    let parent = model
        .entities
        .iter()
        .find(|e| Some(e.id) == fra.parent_id)
        .unwrap();
    assert_eq!(parent.canonical_code, "EU");
    // The collapsed FRA must own both polygons.
    let merged = model.geometry_for_country.get("FRA").unwrap();
    assert_eq!(merged.0.len(), 2);
}

/// A single-feature country IS the whole country, so its own ISO_A3_EH is
/// authoritative even when NE's ADM0_A3 is a non-ISO code and TYPE is not
/// "Country" (e.g. Palestine PSX → PSE, TYPE "Indeterminate").
#[test]
fn single_feature_uses_own_iso_a3_eh_even_when_adm0_differs() {
    use serde_json::json;
    let raw = json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {"ADM0_A3":"PSX","ISO_A3":"PSE","ISO_A3_EH":"PSE","NAME":"Palestine","CONTINENT":"Asia","REGION_UN":"Asia","TYPE":"Indeterminate"},
            "geometry": {"type":"Polygon","coordinates":[[[35.0,31.0],[36.0,31.0],[36.0,32.0],[35.0,32.0],[35.0,31.0]]]}
        }]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_admin0(tmp.path(), "iso").unwrap();
    let registry = Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 0,
            point: None,
        }],
        countries: vec![Entry {
            code: "PSX".into(),
            id: 1,
            point: None,
        }],
        provinces: vec![],
    };
    let model = assemble_entities(&features, &registry).unwrap();
    let psx = model
        .entities
        .iter()
        .find(|e| e.canonical_code == "PSX")
        .expect("PSX entity");
    assert_eq!(psx.iso_a3_eh.as_deref(), Some("PSE"));
}

/// A collapsed group with no TYPE=="Country" member has no single sovereign ISO
/// (the NE `IOA` bucket = Cocos + Christmas, two distinct ISO territories).
#[test]
fn collapsed_group_without_sovereign_member_is_none() {
    use serde_json::json;
    let raw = json!({
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"IOA","ISO_A3":"CCK","ISO_A3_EH":"CCK","NAME":"Cocos Is.","CONTINENT":"Oceania","REGION_UN":"Oceania","TYPE":"Geo unit"},
                "geometry": {"type":"Polygon","coordinates":[[[96.0,-12.0],[97.0,-12.0],[97.0,-11.0],[96.0,-11.0],[96.0,-12.0]]]}
            },
            {
                "type": "Feature",
                "properties": {"ADM0_A3":"IOA","ISO_A3":"CXR","ISO_A3_EH":"CXR","NAME":"Christmas I.","CONTINENT":"Oceania","REGION_UN":"Oceania","TYPE":"Geo unit"},
                "geometry": {"type":"Polygon","coordinates":[[[105.0,-11.0],[106.0,-11.0],[106.0,-10.0],[105.0,-10.0],[105.0,-11.0]]]}
            }
        ]
    });
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), serde_json::to_string(&raw).unwrap()).unwrap();
    let features = parse_admin0(tmp.path(), "iso").unwrap();
    let registry = Registry {
        schema: 1,
        continents: vec![Entry {
            code: "OC".into(),
            id: 0,
            point: None,
        }],
        countries: vec![Entry {
            code: "IOA".into(),
            id: 1,
            point: None,
        }],
        provinces: vec![],
    };
    let model = assemble_entities(&features, &registry).unwrap();
    let ioa = model
        .entities
        .iter()
        .find(|e| e.canonical_code == "IOA")
        .expect("IOA entity");
    assert_eq!(ioa.iso_a3_eh, None);
}

#[test]
fn entity_ids_are_dense_and_continents_first() {
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
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
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model = assemble_entities(&features, &registry).unwrap();
    let _value = model.entities[0].id;
}

fn prov(code: &str, adm0: &str) -> Admin1Feature {
    let sq = Polygon::new(
        LineString(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]),
        vec![],
    );
    Admin1Feature {
        adm1_code: code.into(),
        adm0_a3: adm0.into(),
        iso_3166_2: format!("{adm0}-1"),
        name_en: Some(code.into()),
        name_zh: Some(code.into()),
        geometry: MultiPolygon(vec![sq]),
    }
}

/// Countries `AAA` and `BBB` (both in Asia) plus the given provinces, all keyed
/// through the synthetic registry.
fn model_with(provinces: &[Admin1Feature]) -> EntityModel {
    let features = [feat("AAA", "Asia"), feat("BBB", "Asia")];
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let mut model = assemble_entities(&features, &registry).unwrap();
    collect_provinces(&mut model, provinces, &registry).unwrap();
    model
}

fn country_id(model: &EntityModel, code: &str) -> GeoEntityId {
    model
        .entities
        .iter()
        .find(|e| matches!(e.kind, GeoEntityKind::Country) && e.canonical_code == code)
        .expect("country entity")
        .id
}

/// `[(adm1_code, [(country code, blocks)])]` → the tally the raster would give.
fn tally(model: &EntityModel, entries: &[(&str, &[(&str, u64)])]) -> Coverage {
    let mut out = Coverage::new();
    for (adm1_code, by_country) in entries {
        let slot = model.province_ids[*adm1_code].0 as usize;
        if slot >= out.len() {
            out.resize_with(slot + 1, Default::default);
        }
        out[slot] = by_country
            .iter()
            .map(|(code, blocks)| (country_id(model, code), *blocks))
            .collect();
    }
    out
}

#[test]
fn provinces_become_entities_under_their_parent() {
    let mut model = model_with(&[prov("AAA-001", "AAA")]);
    assert!(model.geometry_for_province.contains_key("AAA-001"));
    assert!(
        !model
            .entities
            .iter()
            .any(|e| matches!(e.kind, GeoEntityKind::Province)),
        "collect_provinces only gathers geometry — parents are not known yet"
    );

    let tally = tally(&model, &[("AAA-001", &[("AAA", 10)])]);
    let parents = resolve_province_parents(&model, &tally, Worldview::Iso).unwrap();
    attach_province_entities(&mut model, &parents);

    let province = model
        .entities
        .iter()
        .find(|e| e.canonical_code == "AAA-001")
        .expect("province entity");
    assert_eq!(province.kind, GeoEntityKind::Province);
    assert_eq!(province.parent_id, Some(country_id(&model, "AAA")));
    assert_eq!(province.name_key, "province.AAA-001");
    assert_eq!(province.iso_a3_eh, None);
    assert!(
        model.entities.windows(2).all(|w| w[0].id.0 <= w[1].id.0),
        "entities must stay sorted by id"
    );
}

/// The defect this rule exists for: Natural Earth's admin-0 excises disputed
/// land from both claimants, leaving the province a residue of coverage that
/// happens to sit in a neighbour. Block majority over that residue used to ship
/// Ladakh under China; the declared `adm0_a3` outranks it.
#[test]
fn the_declared_country_outranks_the_block_majority() {
    let model = model_with(&[prov("AAA-001", "AAA")]);
    let tally = tally(&model, &[("AAA-001", &[("AAA", 1), ("BBB", 100)])]);

    let parents = resolve_province_parents(&model, &tally, Worldview::Iso).unwrap();
    assert_eq!(
        parents[&model.province_ids["AAA-001"]],
        country_id(&model, "AAA"),
    );
}

/// The case block majority was designed for and must keep serving: an
/// `adm0_a3` that is no country in this worldview (Kosovo in `iso`, Hong Kong
/// in `chn`, Somaliland everywhere).
#[test]
fn block_majority_governs_a_declared_code_this_worldview_lacks() {
    let model = model_with(&[prov("AAA-001", "ZZZ")]);
    let tally = tally(&model, &[("AAA-001", &[("AAA", 1), ("BBB", 100)])]);

    let parents = resolve_province_parents(&model, &tally, Worldview::Iso).unwrap();
    assert_eq!(
        parents[&model.province_ids["AAA-001"]],
        country_id(&model, "BBB"),
    );
}

#[test]
fn a_province_with_no_land_at_all_is_an_error() {
    let model = model_with(&[prov("AAA-001", "AAA")]);

    let err = resolve_province_parents(&model, &Coverage::new(), Worldview::Iso)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("AAA-001"),
        "error must name the province: {err}"
    );
    assert!(
        err.contains("UNPARENTED_PROVINCES"),
        "error must tell the author how to allow it deliberately: {err}"
    );
}

/// The declared code wins, but only over land its country actually holds. A
/// province the mask hands entirely to someone else cannot quietly fall back to
/// that someone else — that is exactly the misattribution this rule fixes — so
/// it stops the build and names both remedies.
#[test]
fn a_declared_parent_holding_none_of_the_province_is_an_error() {
    let model = model_with(&[prov("AAA-001", "AAA")]);
    let tally = tally(&model, &[("AAA-001", &[("BBB", 100)])]);

    let err = resolve_province_parents(&model, &tally, Worldview::Iso)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("AAA-001") && err.contains("BBB"),
        "error must name the province and the country the mask prefers: {err}"
    );
    assert!(
        err.contains("MASK_OVERRULES_DECLARED_PARENT"),
        "error must offer the reviewed-override remedy too: {err}"
    );
}

/// An NE pin bump orphans these in bulk and every rediscovery costs a full
/// rasterize, so one run has to report all of them.
#[test]
fn every_landless_province_is_reported_in_one_run() {
    let model = model_with(&[prov("AAA-001", "AAA"), prov("AAA-002", "AAA")]);

    let err = resolve_province_parents(&model, &Coverage::new(), Worldview::Iso)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("AAA-001") && err.contains("AAA-002"),
        "both offenders must appear in one failure: {err}"
    );
}

/// A model holding exactly one province under the real `adm1_code` the skip
/// list keys on. The gate keys on the code, so the real code can stand in for
/// the real province without its geometry.
fn model_with_one_real_province(adm1_code: &str, adm0: &str) -> EntityModel {
    let features = [feat("AAA", "Asia"), feat("BBB", "Asia")];
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let mut model = assemble_entities(&features, &registry).unwrap();
    model
        .province_ids
        .insert(adm1_code.to_string(), GeoEntityId(100));
    model
        .province_declared_adm0
        .insert(adm1_code.to_string(), adm0.to_string());
    model
}

#[test]
fn a_listed_landless_province_is_skipped() {
    // Crimea in the ISO worldview: NE's iso admin-0 leaves it unclaimed, so the
    // country raster has a hole there and its declared country holds none of it.
    let model = model_with_one_real_province("RUS-283", "AAA");

    let parents = resolve_province_parents(&model, &Coverage::new(), Worldview::Iso).unwrap();
    assert!(
        parents.is_empty(),
        "a listed landless province must get no parent"
    );

    let mut model = model;
    attach_province_entities(&mut model, &parents);
    assert!(
        !model
            .entities
            .iter()
            .any(|e| matches!(e.kind, GeoEntityKind::Province)),
        "a listed landless province must produce no entity"
    );
}

#[test]
fn the_skip_list_is_keyed_by_worldview_not_by_code() {
    // The Essequibo under `chn`, whose admin-0 awards it to Guyana. It is not
    // listed there, so a landless one is still a hard error.
    let model = model_with_one_real_province("GUY-680", "AAA");

    let err = resolve_province_parents(&model, &Coverage::new(), Worldview::Chn)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("GUY-680"),
        "error must name the province: {err}"
    );
    assert!(err.contains("chn"), "error must name the worldview: {err}");
}

#[test]
fn a_listed_province_that_gained_land_is_an_error() {
    // The list must not rot: once the country mask covers it, keeping the entry
    // would silently hold a real province out of the asset.
    let model = model_with_one_real_province("RUS-283", "AAA");
    let tally = tally(&model, &[("RUS-283", &[("AAA", 10)])]);

    let err = resolve_province_parents(&model, &tally, Worldview::Iso)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("RUS-283"),
        "error must name the province: {err}"
    );
    assert!(
        err.contains("UNPARENTED_PROVINCES"),
        "error must point at the list to edit: {err}"
    );
}

/// Countries `RUS` and `UKR` plus Crimea, so the reviewed override list can be
/// exercised against the real `(worldview, adm1_code, ADM0_A3)` triple it holds.
fn crimea_model() -> EntityModel {
    let registry = Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 0,
            point: None,
        }],
        countries: vec![
            Entry {
                code: "RUS".into(),
                id: 1,
                point: None,
            },
            Entry {
                code: "UKR".into(),
                id: 2,
                point: None,
            },
        ],
        provinces: vec![Entry {
            code: "RUS-283".into(),
            id: 3,
            point: None,
        }],
    };
    let features = [feat("RUS", "Asia"), feat("UKR", "Asia")];
    let mut model = assemble_entities(&features, &registry).unwrap();
    collect_provinces(&mut model, &[prov("RUS-283", "RUS")], &registry).unwrap();
    model
}

/// `chn`'s admin-0 awards the whole peninsula to Ukraine and leaves Russia not
/// one block of it, so there the mask is the one to believe.
#[test]
fn a_reviewed_override_lets_the_mask_beat_the_declared_parent() {
    let model = crimea_model();
    let tally = tally(&model, &[("RUS-283", &[("UKR", 143191)])]);

    let parents = resolve_province_parents(&model, &tally, Worldview::Chn).unwrap();
    assert_eq!(
        parents[&model.province_ids["RUS-283"]],
        country_id(&model, "UKR"),
    );
}

#[test]
fn an_override_the_mask_no_longer_supports_is_an_error() {
    let model = crimea_model();
    let tally = tally(&model, &[("RUS-283", &[("RUS", 143191)])]);

    let err = resolve_province_parents(&model, &tally, Worldview::Chn)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("RUS-283"),
        "error must name the province: {err}"
    );
    assert!(
        err.contains("MASK_OVERRULES_DECLARED_PARENT"),
        "error must point at the list to edit: {err}"
    );
}

/// The same code under `iso`, which lists no override for it: the declared
/// country wins and, holding none of the province, sends it to the skip list.
#[test]
fn the_override_list_is_keyed_by_worldview_too() {
    let model = crimea_model();
    let tally = tally(&model, &[("RUS-283", &[("UKR", 143191)])]);

    let parents = resolve_province_parents(&model, &tally, Worldview::Iso).unwrap();
    assert!(
        parents.is_empty(),
        "iso lists RUS-283 as landless, not as mask-overruled"
    );
}
