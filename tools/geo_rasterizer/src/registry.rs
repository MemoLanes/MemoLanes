//! Frozen `ADM0_A3 → GeoEntityId` registry. APPEND ONLY: ids are never
//! renumbered or reused, so they are worldview-invariant and stable
//! across Natural Earth bumps.
//!
//! TODO: Phase 2 (base+delta) reuses this registry unchanged — the
//! entities table is the union across all worldview files, so per-worldview delta
//! sections reference the same ids.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use geo::Centroid;
use geo_data_format::GeoEntityId;
use geo_types::MultiPolygon;
use serde::{Deserialize, Serialize};

// TODO(i18n): an entity's display name is carried as a Flutter l10n KEY, not a
// string — `entities.rs` derives `country.<ADM0_A3>.name` and
// `continent.<code>.name` from these codes (worldviews use `worldview.<id>.name`
// in geo_data_format's `worldview.rs`). The missing piece is the translations: every
// generated key needs an entry in `app/assets/translations/*.json` for each
// locale, ideally checked so a newly registered code can't ship without its
// localized name.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Entry {
    /// Continent 2-letter code or country `ADM0_A3`.
    pub code: String,
    pub id: u32,
    /// Per-worldview representative point: worldview-id -> [lon, lat]. A code is
    /// present only for worldviews whose source file contains it. BTreeMap for
    /// deterministic on-disk ordering.
    #[serde(default)]
    pub refs: std::collections::BTreeMap<String, [f64; 2]>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Registry {
    // TODO: reject schema != 1 once a v2 format exists.
    pub schema: u32,
    #[serde(default, rename = "continent")]
    pub continents: Vec<Entry>,
    #[serde(default, rename = "country")]
    pub countries: Vec<Entry>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading registry at {}", path.display()))?;
        Self::from_toml_str(&raw).with_context(|| format!("parsing registry at {}", path.display()))
    }

    /// Parse the on-disk (compact) form back into the in-memory model:
    /// a single `ref` (expanded to `worldview`, or to the `worldviews` universe when
    /// `worldview` is absent), or an explicit per-worldview `refs` table. Entries with
    /// no point at all load with empty refs.
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let disk: DiskRegistry = toml::from_str(raw).context("parsing registry TOML")?;
        let universe = &disk.worldviews;
        let reg = Registry {
            schema: disk.schema,
            continents: disk
                .continents
                .into_iter()
                .map(|d| d.expand(universe))
                .collect(),
            countries: disk
                .countries
                .into_iter()
                .map(|d| d.expand(universe))
                .collect(),
        };
        reg.validate_unique_ids()?;
        Ok(reg)
    }

    /// No id appears twice across continents+countries (corruption guard).
    pub fn validate_unique_ids(&self) -> Result<()> {
        let mut seen = BTreeMap::new();
        for e in self.continents.iter().chain(self.countries.iter()) {
            if let Some(prev) = seen.insert(e.id, e.code.clone()) {
                bail!("registry: id {} used by both {} and {}", e.id, prev, e.code);
            }
        }
        Ok(())
    }

    fn lookup<'a>(list: &'a [Entry], code: &str) -> Option<&'a Entry> {
        list.iter().find(|e| e.code == code)
    }

    pub fn id_for_continent(&self, code: &str) -> Result<GeoEntityId> {
        Self::lookup(&self.continents, code)
            .map(|e| GeoEntityId(e.id))
            .ok_or_else(|| {
                anyhow!("registry: unknown continent code `{code}` (append it via registry_gen)")
            })
    }

    pub fn id_for_country(&self, adm0_a3: &str) -> Result<GeoEntityId> {
        Self::lookup(&self.countries, adm0_a3)
            .map(|e| GeoEntityId(e.id))
            .ok_or_else(|| {
                anyhow!("registry: unknown ADM0_A3 `{adm0_a3}` (append it via registry_gen)")
            })
    }

    /// One past the current maximum id (next append slot). Returns 0 if empty.
    pub fn next_id(&self) -> u32 {
        self.continents
            .iter()
            .chain(self.countries.iter())
            .map(|e| e.id)
            .max()
            .map_or(0, |m| {
                m.checked_add(1).expect("registry id space exhausted")
            })
    }
}

/// Representative point of a country/continent geometry: the centroid of
/// its merged `MultiPolygon`. Deterministic for a fixed geometry.
pub fn centroid_of(mp: &MultiPolygon<f64>) -> Option<(f64, f64)> {
    mp.centroid().map(|p| (p.x(), p.y()))
}

/// Group `(code, is_continent, geometry)` by `code`, merging all
/// geometries sharing a code into one MultiPolygon; return one
/// `(code, is_continent, (lon, lat))` per code in first-seen order, the
/// point being the centroid of the MERGED geometry. Order-independent:
/// insensitive to feature order within/across input files, so multi-part
/// entities (FRA+overseas, USA+territories, …) get a worldview-stable point.
/// Precondition: all items sharing a `code` must have the same
/// `is_continent`; only the first occurrence's flag is retained.
pub fn merged_representative_points(
    items: impl IntoIterator<Item = (String, bool, geo_types::MultiPolygon<f64>)>,
) -> Vec<(String, bool, (f64, f64))> {
    use std::collections::HashMap;
    let mut order: Vec<String> = Vec::new();
    let mut acc: HashMap<String, (bool, Vec<geo_types::Polygon<f64>>)> = HashMap::new();
    for (code, is_cont, mp) in items {
        let entry = acc.entry(code.clone()).or_insert_with(|| {
            order.push(code.clone());
            (is_cont, Vec::new())
        });
        debug_assert_eq!(
            entry.0, is_cont,
            "code `{code}` appeared with inconsistent is_continent (continent vs country namespace collision)"
        );
        entry.1.extend(mp.0);
    }
    order
        .into_iter()
        .filter_map(|code| {
            let (is_cont, polys) = acc.remove(&code).expect("ordered code must be in acc");
            centroid_of(&geo_types::MultiPolygon(polys)).map(|pt| (code, is_cont, pt))
        })
        .collect()
}

/// Append-only id allocation + per-worldview ref recording. `points` is one
/// entry per code in first-seen order (from `merged_representative_points`
/// for ONE worldview's features). For each `(code, is_continent, (lon,lat))`:
/// on first sight of `code` anywhere, allocate `next_id()` and push a new
/// Entry into `continents` or `countries` per `is_continent` (this
/// preserves first-seen order ⇒ stable ids); then set
/// `entry.refs.insert(worldview.to_string(), [lon, lat])` (insert-or-overwrite
/// for that worldview).
pub fn register_worldview(
    reg: &mut Registry,
    worldview: &str,
    points: &[(String, bool, (f64, f64))],
) {
    for (code, is_continent, (lon, lat)) in points {
        // Find the entry across both vecs; if absent, create with next_id().
        let found = reg
            .continents
            .iter_mut()
            .chain(reg.countries.iter_mut())
            .find(|e| &e.code == code);
        let entry = if let Some(e) = found {
            e
        } else {
            let id = reg.next_id();
            let new_entry = Entry {
                code: code.clone(),
                id,
                refs: std::collections::BTreeMap::new(),
            };
            if *is_continent {
                reg.continents.push(new_entry);
                reg.continents.last_mut().expect("just pushed")
            } else {
                reg.countries.push(new_entry);
                reg.countries.last_mut().expect("just pushed")
            }
        };
        entry.refs.insert(worldview.to_string(), [*lon, *lat]);
    }
}

/// Decimal places kept for representative points. The identity audit
/// compares them with a whole-degrees tolerance, so ~11 m precision is
/// orders of magnitude tighter than needed while keeping the file stable
/// across Natural Earth bumps that don't move a border.
const REF_DECIMALS_FACTOR: f64 = 1e4;

fn round_pt(p: [f64; 2]) -> [f64; 2] {
    [
        (p[0] * REF_DECIMALS_FACTOR).round() / REF_DECIMALS_FACTOR,
        (p[1] * REF_DECIMALS_FACTOR).round() / REF_DECIMALS_FACTOR,
    ]
}

/// Compact on-disk entry. An entity whose every present worldview shares the
/// same (rounded) point stores a single `ref`; `worldview` lists the covered
/// worldviews only when they're a strict subset of the registry-wide `worldviews`
/// universe (so audit coverage is preserved without fabricating refs).
/// Entities that genuinely differ per worldview use the explicit `refs` table.
#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskEntry {
    code: String,
    id: u32,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    ref_pt: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    worldview: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    refs: std::collections::BTreeMap<String, [f64; 2]>,
}

impl DiskEntry {
    fn from_entry(e: &Entry, universe: &[String]) -> Self {
        let rounded: std::collections::BTreeMap<String, [f64; 2]> = e
            .refs
            .iter()
            .map(|(k, v)| (k.clone(), round_pt(*v)))
            .collect();
        let mut base = DiskEntry {
            code: e.code.clone(),
            id: e.id,
            ..Default::default()
        };
        let mut distinct: Vec<[f64; 2]> = Vec::new();
        for v in rounded.values() {
            if !distinct.contains(v) {
                distinct.push(*v);
            }
        }
        match distinct.len() {
            0 => {}
            1 => {
                base.ref_pt = Some(distinct[0]);
                let keys: Vec<String> = rounded.keys().cloned().collect();
                if keys != universe {
                    base.worldview = keys;
                }
            }
            _ => base.refs = rounded,
        }
        base
    }

    fn expand(self, universe: &[String]) -> Entry {
        let refs = if !self.refs.is_empty() {
            self.refs
        } else if let Some(pt) = self.ref_pt {
            let keys = if self.worldview.is_empty() {
                universe
            } else {
                &self.worldview
            };
            keys.iter().map(|k| (k.clone(), pt)).collect()
        } else {
            std::collections::BTreeMap::new()
        };
        Entry {
            code: self.code,
            id: self.id,
            refs,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskRegistry {
    schema: u32,
    /// Registry-wide worldview universe (sorted union of all entry refs). A bare
    /// `ref` with no per-entry `worldview` expands to exactly these on load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    worldviews: Vec<String>,
    #[serde(default, rename = "continent")]
    continents: Vec<DiskEntry>,
    #[serde(default, rename = "country")]
    countries: Vec<DiskEntry>,
}

/// Stable on-disk form: entries sorted by `code` for human review, points
/// rounded ([`REF_DECIMALS_FACTOR`]) and worldview-identical entries collapsed.
/// IDs are explicit fields, so sorting/collapsing never changes any id.
pub fn to_toml_sorted(reg: &Registry) -> Result<String> {
    let mut universe: Vec<String> = reg
        .continents
        .iter()
        .chain(reg.countries.iter())
        .flat_map(|e| e.refs.keys().cloned())
        .collect();
    universe.sort();
    universe.dedup();

    let to_disk = |list: &[Entry]| {
        let mut v: Vec<DiskEntry> = list
            .iter()
            .map(|e| DiskEntry::from_entry(e, &universe))
            .collect();
        v.sort_by(|a, b| a.code.cmp(&b.code));
        v
    };
    let disk = DiskRegistry {
        schema: reg.schema,
        continents: to_disk(&reg.continents),
        countries: to_disk(&reg.countries),
        worldviews: universe,
    };
    toml::to_string(&disk).context("serializing registry")
}

/// CI gate 2 — identity audit. For every `(code, centroid)` in `present`:
/// find the Entry by code (continents or countries). If found AND
/// `entry.refs.get(worldview)` is `Some([rlon, rlat])`: compute Euclidean degree
/// distance; if `> tol_deg` → bail (message includes code, worldview, distance,
/// and both points). If the code is not in the registry OR has no ref for
/// `worldview` → skip (Ok). A registry-absent code is ignored here (Task 3 /
/// unknown-code gate owns that). Tolerance is intentionally generous: it
/// must NOT trip on normal per-worldview boundary moves, only on a code denoting
/// a different place entirely.
pub fn audit_identity(
    present: &[(String, (f64, f64))],
    registry: &Registry,
    worldview: &str,
    tol_deg: f64,
) -> Result<()> {
    for (code, (lon, lat)) in present {
        let entry = Registry::lookup(&registry.continents, code)
            .or_else(|| Registry::lookup(&registry.countries, code));
        let Some(e) = entry else { continue };
        let Some([rlon, rlat]) = e.refs.get(worldview) else {
            continue;
        };
        let dlon = lon - rlon;
        let dlat = lat - rlat;
        let dist = (dlon * dlon + dlat * dlat).sqrt();
        if dist > tol_deg {
            bail!(
                "identity audit: `{code}` (worldview={worldview}) centroid ({lon:.2},{lat:.2}) is \
                 {dist:.2}° from registry reference ({rlon:.2},{rlat:.2}); a code must \
                 denote the same place across worldviews/bumps — investigate before bumping"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    use std::collections::BTreeMap;

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
}
