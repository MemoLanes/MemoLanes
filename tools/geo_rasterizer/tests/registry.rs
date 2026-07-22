//! Unit-level tests for the frozen id registry (moved out of `registry.rs` so
//! `src/` carries no inline test modules). Every item exercised here is part of
//! the crate's public API, so no visibility had to be widened.

use std::collections::BTreeMap;

use geo_data_format::GeoEntityId;
use geo_rasterizer::registry::{
    audit_identity, merged_representative_points, register_worldview, to_toml_sorted, Entry,
    Registry,
};

fn sample() -> Registry {
    Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 0,
            refs: std::collections::BTreeMap::from([("iso".to_string(), [100.0, 30.0])]),
        }],
        countries: vec![Entry {
            code: "USA".into(),
            id: 7,
            refs: std::collections::BTreeMap::from([("iso".to_string(), [-98.5, 39.5])]),
        }],
    }
}

#[test]
fn lookups_and_next_id() {
    let r = sample();
    assert_eq!(r.id_for_continent("AS").unwrap(), GeoEntityId(0));
    assert_eq!(r.id_for_country("USA").unwrap(), GeoEntityId(7));
    assert!(r.id_for_country("XXX").is_err());
    assert!(r.id_for_continent("UNKNOWN").is_err());
    assert_eq!(r.next_id(), 8);
    r.validate_unique_ids().unwrap();
}

#[test]
fn duplicate_id_rejected() {
    let mut r = sample();
    r.countries.push(Entry {
        code: "CAN".into(),
        id: 0,
        refs: Default::default(),
    });
    assert!(r.validate_unique_ids().is_err());
    let msg = r.validate_unique_ids().unwrap_err().to_string();
    assert!(msg.contains("CAN") && msg.contains("AS"), "got: {msg}");
}

#[test]
fn identity_audit_passes_when_close_and_fails_when_far() {
    let r = sample();
    // (i) USA ref for "iso" is (-98.5, 39.5). Within tolerance → ok.
    audit_identity(&[("USA".into(), (-97.0, 40.0))], &r, "iso", 5.0).unwrap();
    // (ii) Same code, centroid in Asia → code reused for a different place.
    let err = audit_identity(&[("USA".into(), (100.0, 30.0))], &r, "iso", 5.0)
        .unwrap_err()
        .to_string();
    assert!(err.contains("USA"), "got: {err}");
    assert!(
        err.contains("iso"),
        "msg must include worldview; got: {err}"
    );
    // (iii) Code present in registry but no ref for the queried worldview → skip (Ok).
    audit_identity(&[("USA".into(), (100.0, 30.0))], &r, "chn", 5.0).unwrap();
    // (iv) Code absent from registry → skip (Ok).
    audit_identity(&[("ZZZ".into(), (0.0, 0.0))], &r, "iso", 5.0).unwrap();
}

#[test]
fn register_worldview_appends_and_sets_per_worldview_refs() {
    let mut r = sample(); // AS=0(iso), USA=7(iso)  next_id=8
                          // iso pass: USA already exists (id frozen), CAN is new.
    register_worldview(
        &mut r,
        "iso",
        &[
            ("USA".to_string(), false, (-97.0, 40.0)),
            ("CAN".to_string(), false, (-106.0, 56.0)),
        ],
    );
    let usa = r.countries.iter().find(|e| e.code == "USA").unwrap();
    assert_eq!(usa.id, 7, "existing id must never change");
    // ref for "iso" is now updated to the new point (insert-or-overwrite).
    assert_eq!(
        usa.refs.get("iso"),
        Some(&[-97.0_f64, 40.0_f64]),
        "iso ref updated"
    );
    let can = r.countries.iter().find(|e| e.code == "CAN").unwrap();
    assert_eq!(can.id, 8, "new code gets next_id");
    assert_eq!(can.refs.get("iso"), Some(&[-106.0_f64, 56.0_f64]));
    r.validate_unique_ids().unwrap();

    // Second worldview: "chn" adds its own ref for CAN without changing ids.
    let can_id_before = can.id;
    register_worldview(&mut r, "chn", &[("CAN".to_string(), false, (-105.0, 55.0))]);
    let can = r.countries.iter().find(|e| e.code == "CAN").unwrap();
    assert_eq!(can.id, can_id_before, "id unchanged by second worldview");
    assert_eq!(can.refs.get("chn"), Some(&[-105.0_f64, 55.0_f64]));
    assert_eq!(
        can.refs.get("iso"),
        Some(&[-106.0_f64, 56.0_f64]),
        "iso ref unaffected"
    );
    r.validate_unique_ids().unwrap();

    // A brand-new continent is appended to `continents` with next_id.
    let prev = r.next_id();
    register_worldview(&mut r, "iso", &[("EU".to_string(), true, (10.0, 50.0))]);
    let eu = r.continents.iter().find(|e| e.code == "EU").unwrap();
    assert_eq!(eu.id, prev, "new continent gets next_id");
    assert_eq!(eu.refs.get("iso"), Some(&[10.0_f64, 50.0_f64]));
    r.validate_unique_ids().unwrap();
}

#[test]
fn audit_identity_matches_continent_codes_too() {
    let r = sample(); // continent AS has iso ref at (100.0, 30.0)
                      // Far from AS reference for "iso" → must fail.
    let err = audit_identity(&[("AS".into(), (0.0, 0.0))], &r, "iso", 5.0)
        .unwrap_err()
        .to_string();
    assert!(err.contains("AS"), "got: {err}");
}

#[test]
fn merged_repr_point_is_order_independent() {
    use geo_types::{Coord, LineString, MultiPolygon, Polygon};
    fn sq(x0: f64, y0: f64) -> MultiPolygon<f64> {
        MultiPolygon(vec![Polygon::new(
            LineString(vec![
                Coord { x: x0, y: y0 },
                Coord { x: x0 + 1.0, y: y0 },
                Coord {
                    x: x0 + 1.0,
                    y: y0 + 1.0,
                },
                Coord { x: x0, y: y0 },
            ]),
            vec![],
        )])
    }
    // Code "FR" split into two far-apart parts, fed in BOTH orders.
    let a = merged_representative_points(vec![
        ("FR".to_string(), false, sq(0.0, 0.0)),
        ("FR".to_string(), false, sq(100.0, 0.0)),
    ]);
    let b = merged_representative_points(vec![
        ("FR".to_string(), false, sq(100.0, 0.0)),
        ("FR".to_string(), false, sq(0.0, 0.0)),
    ]);
    assert_eq!(a.len(), 1);
    assert_eq!(
        a, b,
        "representative point must not depend on feature order"
    );
    // Merged point differs from either single-part centroid.
    let single = merged_representative_points(vec![("FR".to_string(), false, sq(0.0, 0.0))]);
    assert_ne!(a[0].2, single[0].2);
    // First-seen order of distinct codes preserved; is_continent kept from first.
    let multi = merged_representative_points(vec![
        ("AS".to_string(), true, sq(0.0, 0.0)),
        ("AAA".to_string(), false, sq(5.0, 5.0)),
        ("AS".to_string(), true, sq(2.0, 2.0)),
    ]);
    assert_eq!(
        multi.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
        vec!["AS", "AAA"]
    );
    assert!(multi[0].1 && !multi[1].1);
}

fn bt(pairs: &[(&str, [f64; 2])]) -> BTreeMap<String, [f64; 2]> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

/// #2+#3: an entity whose every present worldview shares the same point
/// collapses to a single inline `ref` (no `ref_*`, no `[*.refs]`
/// sub-table, no exploded multi-line arrays) and round-trips back to
/// the full per-worldview map.
#[test]
fn compact_collapses_identical_full_universe() {
    let reg = Registry {
        schema: 1,
        continents: vec![Entry {
            code: "AS".into(),
            id: 0,
            refs: bt(&[
                ("chn", [100.0, 30.0]),
                ("iso", [100.0, 30.0]),
                ("usa", [100.0, 30.0]),
            ]),
        }],
        countries: vec![Entry {
            code: "USA".into(),
            id: 7,
            refs: bt(&[
                ("chn", [-98.5, 39.5]),
                ("iso", [-98.5, 39.5]),
                ("usa", [-98.5, 39.5]),
            ]),
        }],
    };
    let txt = to_toml_sorted(&reg).unwrap();
    assert!(
        !txt.contains("[\n"),
        "arrays must be inline (#3); got:\n{txt}"
    );
    assert!(
        !txt.contains("ref_"),
        "identical worldviews must collapse (#2)"
    );
    assert!(
        !txt.contains(".refs]"),
        "no per-worldview sub-table when identical"
    );
    assert!(
        txt.contains("ref = ["),
        "expected single inline ref; got:\n{txt}"
    );

    let back = Registry::from_toml_str(&txt).unwrap();
    let usa = back.countries.iter().find(|e| e.code == "USA").unwrap();
    assert_eq!(usa.id, 7);
    assert_eq!(
        usa.refs,
        bt(&[
            ("chn", [-98.5, 39.5]),
            ("iso", [-98.5, 39.5]),
            ("usa", [-98.5, 39.5])
        ])
    );
}

/// #2: an entity present in only a subset of worldviews records that subset
/// and round-trips to exactly those keys — no fabricated refs (audit
/// coverage must be preserved exactly).
#[test]
fn compact_preserves_worldview_subset_without_fabrication() {
    let reg = Registry {
        schema: 1,
        continents: vec![],
        countries: vec![
            Entry {
                code: "AAA".into(),
                id: 0,
                refs: bt(&[
                    ("chn", [1.0, 2.0]),
                    ("iso", [1.0, 2.0]),
                    ("usa", [1.0, 2.0]),
                ]),
            },
            Entry {
                code: "BBB".into(),
                id: 1,
                refs: bt(&[("usa", [5.0, 6.0])]),
            },
        ],
    };
    let back = Registry::from_toml_str(&to_toml_sorted(&reg).unwrap()).unwrap();
    let bbb = back.countries.iter().find(|e| e.code == "BBB").unwrap();
    assert_eq!(
        bbb.refs,
        bt(&[("usa", [5.0, 6.0])]),
        "subset entity must not gain chn/iso on round-trip"
    );
}

/// #2: differing per-worldview points are kept distinct across the round-trip.
#[test]
fn compact_keeps_differing_per_worldview_points() {
    let reg = Registry {
        schema: 1,
        continents: vec![],
        countries: vec![Entry {
            code: "DIS".into(),
            id: 3,
            refs: bt(&[
                ("chn", [35.1, 31.4]),
                ("iso", [34.9, 31.0]),
                ("usa", [34.8, 31.2]),
            ]),
        }],
    };
    let back = Registry::from_toml_str(&to_toml_sorted(&reg).unwrap()).unwrap();
    let dis = back.countries.iter().find(|e| e.code == "DIS").unwrap();
    assert_eq!(
        dis.refs,
        bt(&[
            ("chn", [35.1, 31.4]),
            ("iso", [34.9, 31.0]),
            ("usa", [34.8, 31.2])
        ])
    );
}

/// #1: points are rounded to 4 dp on disk, and the serialized form is
/// idempotent (re-emitting a parsed registry is byte-identical — a
/// Natural Earth bump that doesn't move a border yields a zero-line
/// diff).
#[test]
fn refs_rounded_to_4dp_and_idempotent() {
    let reg = Registry {
        schema: 1,
        continents: vec![],
        countries: vec![Entry {
            code: "PRC".into(),
            id: 0,
            refs: bt(&[("chn", [29.851884627, -19.002536684])]),
        }],
    };
    let txt = to_toml_sorted(&reg).unwrap();
    let parsed = Registry::from_toml_str(&txt).unwrap();
    let prc = parsed.countries.iter().find(|e| e.code == "PRC").unwrap();
    assert_eq!(
        prc.refs.get("chn"),
        Some(&[29.8519, -19.0025]),
        "stored point must be rounded to 4 dp"
    );
    assert_eq!(
        to_toml_sorted(&parsed).unwrap(),
        txt,
        "serialization must be idempotent"
    );
}

/// A refs-less registry (the synthetic test fixture: code + id only,
/// audit not exercised) loads with intact ids and empty refs, so the
/// golden / rasterize / entities / area / cache fixtures keep working.
#[test]
fn refsless_fixture_loads() {
    let src = "schema = 1\n\n\
         [[continent]]\ncode = \"AF\"\nid = 0\n\n\
         [[country]]\ncode = \"AAA\"\nid = 3\n";
    let reg = Registry::from_toml_str(src).unwrap();
    assert_eq!(reg.id_for_continent("AF").unwrap(), GeoEntityId(0));
    assert_eq!(reg.id_for_country("AAA").unwrap(), GeoEntityId(3));
    assert!(reg.continents[0].refs.is_empty());
}
