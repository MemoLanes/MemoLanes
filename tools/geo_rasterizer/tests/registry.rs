//! Registry behaviour: append-only id assignment, representative-point
//! re-baselining, and the movement primitives used by the bump-time report.

use geo_data_format::GeoEntityId;
use geo_rasterizer::registry::{register_worldview, to_toml_sorted, Entry, Namespace, Registry};

fn country(code: &str, id: u32, point: Option<[f64; 2]>) -> Entry {
    Entry {
        code: code.to_string(),
        id,
        point,
    }
}

fn sample() -> Registry {
    Registry {
        schema: 1,
        // Continent: no point — identity is the code.
        continents: vec![country("AS", 0, None)],
        countries: vec![country("USA", 7, Some([-97.0, 40.0]))],
        provinces: vec![],
    }
}

#[test]
fn lookups_and_next_id() {
    let r = sample();
    assert_eq!(r.id_for_continent("AS").unwrap().0, 0);
    assert_eq!(r.id_for_country("USA").unwrap().0, 7);
    assert!(r.id_for_country("ZZZ").is_err());
    assert_eq!(r.next_id(), 8);
}

#[test]
fn duplicate_id_rejected() {
    let r = Registry {
        schema: 1,
        continents: vec![country("AS", 5, None)],
        countries: vec![country("USA", 5, None)],
        provinces: vec![],
    };
    assert!(r.validate_unique_ids().is_err());
}

#[test]
fn register_appends_ids_and_sets_country_points() {
    let mut r = sample(); // AS=0 (continent), USA=7  next_id=8
    register_worldview(
        &mut r,
        &[
            ("EU".to_string(), Namespace::Continent, (10.0, 50.0)), // new continent
            ("CAN".to_string(), Namespace::Country, (-106.0, 56.0)), // new country
        ],
    );

    let eu = r.continents.iter().find(|e| e.code == "EU").unwrap();
    assert_eq!(eu.id, 8);
    assert_eq!(eu.point, None, "continents carry no representative point");

    let can = r.countries.iter().find(|e| e.code == "CAN").unwrap();
    assert_eq!(can.id, 9);
    assert_eq!(can.point, Some([-106.0, 56.0]));
}

#[test]
fn register_rebaselines_existing_point_but_keeps_id() {
    let mut r = sample(); // USA=7 at (-97, 40)
    register_worldview(
        &mut r,
        &[("USA".to_string(), Namespace::Country, (-98.5, 39.5))],
    );
    let usa = r.countries.iter().find(|e| e.code == "USA").unwrap();
    assert_eq!(usa.id, 7, "id is frozen");
    assert_eq!(
        usa.point,
        Some([-98.5, 39.5]),
        "point re-baselines to current geometry (it is a heuristic, not frozen)"
    );
    assert_eq!(r.next_id(), 8, "no new id for an existing code");
}

#[test]
fn toml_round_trip_preserves_point() {
    let r = sample();
    let toml = to_toml_sorted(&r).unwrap();
    let back = Registry::from_toml_str(&toml).unwrap();
    assert_eq!(back, r);
    // Only the country carries a point; the continent serializes without it.
    assert_eq!(toml.matches("point = ").count(), 1);
}

#[test]
fn province_ids_resolve_and_stay_unique() {
    let toml = r#"
schema = 1

[[continent]]
code = "EU"
id = 0

[[country]]
code = "AAA"
id = 1

[[province]]
code = "AAA-001"
id = 2
point = [0.5, 0.75]
"#;
    let reg = Registry::from_toml_str(toml).unwrap();
    assert_eq!(reg.id_for_province("AAA-001").unwrap(), GeoEntityId(2));
    assert_eq!(reg.next_id(), 3);
    let err = reg.id_for_province("AAA-999").unwrap_err().to_string();
    assert!(err.contains("AAA-999"), "got: {err}");
    assert!(
        err.contains("registry_gen"),
        "error must say how to fix it: {err}"
    );
}

#[test]
fn duplicate_id_across_namespaces_is_rejected() {
    let toml = r#"
schema = 1

[[country]]
code = "AAA"
id = 7

[[province]]
code = "AAA-001"
id = 7
"#;
    let err = Registry::from_toml_str(toml).unwrap_err().to_string();
    assert!(err.contains("id 7"), "got: {err}");
}
