//! Registry behaviour: append-only id assignment, representative-point
//! re-baselining, the movement primitives used by the bump-time report, and
//! the on-disk split into one file per namespace.

use geo_data_format::GeoEntityId;
use geo_rasterizer::registry::{register_worldview, Entry, Namespace, Registry};

fn entry(code: &str, id: u32, point: Option<[f64; 2]>) -> Entry {
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
        continents: vec![entry("AS", 0, None)],
        countries: vec![entry("USA", 7, Some([-97.0, 40.0]))],
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
        continents: vec![entry("AS", 5, None)],
        countries: vec![entry("USA", 5, None)],
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
fn dir_round_trip_preserves_entries() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = sample();
    r.provinces.push(entry("USA-001", 9, Some([-97.5, 40.5])));
    r.write(dir.path()).unwrap();
    assert_eq!(Registry::load(dir.path()).unwrap(), r);
}

#[test]
fn each_namespace_gets_its_own_file_one_line_per_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = sample();
    r.provinces.push(entry("USA-001", 9, Some([-97.5, 40.5])));
    r.write(dir.path()).unwrap();

    let read = |name: &str| std::fs::read_to_string(dir.path().join(name)).unwrap();
    assert_eq!(
        read("continents.toml"),
        "schema = 1\n\ncontinent = [\n  { code = \"AS\", id = 0 },\n]\n"
    );
    assert_eq!(
        read("countries.toml"),
        "schema = 1\n\ncountry = [\n  { code = \"USA\", id = 7, point = [-97.0, 40.0] },\n]\n"
    );
    // A namespace file never carries another namespace's entries.
    assert!(!read("countries.toml").contains("USA-001"));
}

#[test]
fn codes_needing_quoting_round_trip() {
    // 33 real `adm1_code`s are not bare-key-shaped, e.g. Antarctica's `ATA+00?`.
    let dir = tempfile::tempdir().unwrap();
    let mut r = sample();
    r.provinces.push(entry("ATA+00?", 9, Some([0.0, -90.0])));
    r.write(dir.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("provinces.toml")).unwrap(),
        "schema = 1\n\nprovince = [\n  { code = \"ATA+00?\", id = 9, point = [0.0, -90.0] },\n]\n"
    );
    assert_eq!(Registry::load(dir.path()).unwrap(), r);
}

#[test]
fn empty_namespace_file_is_still_a_valid_document() {
    let dir = tempfile::tempdir().unwrap();
    sample().write(dir.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("provinces.toml")).unwrap(),
        "schema = 1\n\nprovince = []\n"
    );
    assert!(Registry::load(dir.path()).unwrap().provinces.is_empty());
}

#[test]
fn load_rejects_disagreeing_schemas() {
    let dir = tempfile::tempdir().unwrap();
    sample().write(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("provinces.toml"),
        "schema = 2\n\nprovince = []\n",
    )
    .unwrap();
    let err = Registry::load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("schema 2"), "got: {err}");
}

#[test]
fn province_ids_resolve_and_stay_unique() {
    let toml = r#"
schema = 1

continent = [{ code = "EU", id = 0 }]
country = [{ code = "AAA", id = 1 }]
province = [{ code = "AAA-001", id = 2, point = [0.5, 0.75] }]
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

country = [{ code = "AAA", id = 7 }]
province = [{ code = "AAA-001", id = 7 }]
"#;
    let err = Registry::from_toml_str(toml).unwrap_err().to_string();
    assert!(err.contains("id 7"), "got: {err}");
}
