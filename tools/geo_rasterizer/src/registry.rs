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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Entry {
    /// Continent 2-letter code, country `ADM0_A3`, or province `adm1_code`.
    pub code: String,
    pub id: u32,
    /// Representative point `[lon, lat]` (union centroid, merged across all
    /// worldviews), re-baselined each regen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespace {
    Continent,
    Country,
    Province,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Registry {
    // TODO: reject schema != 1 once a v2 format exists.
    pub schema: u32,
    #[serde(default, rename = "continent")]
    pub continents: Vec<Entry>,
    #[serde(default, rename = "country")]
    pub countries: Vec<Entry>,
    #[serde(default, rename = "province")]
    pub provinces: Vec<Entry>,
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading registry at {}", path.display()))?;
        Self::from_toml_str(&raw).with_context(|| format!("parsing registry at {}", path.display()))
    }

    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let reg: Registry = toml::from_str(raw).context("parsing registry TOML")?;
        reg.validate_unique_ids()?;
        Ok(reg)
    }

    /// No id appears twice across continents+countries+provinces (corruption guard).
    pub fn validate_unique_ids(&self) -> Result<()> {
        let mut seen = BTreeMap::new();
        for e in self
            .continents
            .iter()
            .chain(self.countries.iter())
            .chain(self.provinces.iter())
        {
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

    pub fn id_for_province(&self, adm1_code: &str) -> Result<GeoEntityId> {
        Self::lookup(&self.provinces, adm1_code)
            .map(|e| GeoEntityId(e.id))
            .ok_or_else(|| {
                anyhow!("registry: unknown adm1_code `{adm1_code}` (append it via registry_gen)")
            })
    }

    /// One past the current maximum id (next append slot). Returns 0 if empty.
    pub fn next_id(&self) -> u32 {
        self.continents
            .iter()
            .chain(self.countries.iter())
            .chain(self.provinces.iter())
            .map(|e| e.id)
            .max()
            .map_or(0, |m| {
                m.checked_add(1).expect("registry id space exhausted")
            })
    }
}

pub fn centroid_of(mp: &MultiPolygon<f64>) -> Option<(f64, f64)> {
    mp.centroid().map(|p| (p.x(), p.y()))
}

const POINT_DECIMALS_FACTOR: f64 = 1e4;

fn round_point([lon, lat]: [f64; 2]) -> [f64; 2] {
    [
        (lon * POINT_DECIMALS_FACTOR).round() / POINT_DECIMALS_FACTOR,
        (lat * POINT_DECIMALS_FACTOR).round() / POINT_DECIMALS_FACTOR,
    ]
}

pub fn admin0_point_items(
    features: &[crate::admin0::Admin0Feature],
) -> Vec<(String, Namespace, MultiPolygon<f64>)> {
    let mut items = Vec::with_capacity(features.len() * 2);
    for f in features {
        let continent = crate::entities::feature_continent_code(&f.continent, &f.region_un);
        items.push((
            continent.to_string(),
            Namespace::Continent,
            f.geometry.clone(),
        ));
        items.push((f.adm0_a3.clone(), Namespace::Country, f.geometry.clone()));
    }
    items
}

pub fn admin1_point_items(
    features: &[crate::admin1::Admin1Feature],
) -> Vec<(String, Namespace, MultiPolygon<f64>)> {
    features
        .iter()
        .map(|f| (f.adm1_code.clone(), Namespace::Province, f.geometry.clone()))
        .collect()
}

pub fn merged_geometries(
    items: impl IntoIterator<Item = (String, Namespace, MultiPolygon<f64>)>,
) -> Vec<(String, Namespace, MultiPolygon<f64>)> {
    use std::collections::HashMap;
    let mut order: Vec<String> = Vec::new();
    let mut acc: HashMap<String, (Namespace, Vec<geo_types::Polygon<f64>>)> = HashMap::new();
    for (code, namespace, mp) in items {
        let entry = acc.entry(code.clone()).or_insert_with(|| {
            order.push(code.clone());
            (namespace, Vec::new())
        });
        debug_assert_eq!(
            entry.0, namespace,
            "code `{code}` appeared with inconsistent namespace (cross-namespace code collision)"
        );
        entry.1.extend(mp.0);
    }
    order
        .into_iter()
        .map(|code| {
            let (namespace, polys) = acc.remove(&code).expect("ordered code must be in acc");
            (code, namespace, MultiPolygon(polys))
        })
        .collect()
}

pub fn merged_representative_points(
    items: impl IntoIterator<Item = (String, Namespace, MultiPolygon<f64>)>,
) -> Vec<(String, Namespace, (f64, f64))> {
    merged_geometries(items)
        .into_iter()
        .filter_map(|(code, namespace, mp)| centroid_of(&mp).map(|pt| (code, namespace, pt)))
        .collect()
}

pub fn register_worldview(reg: &mut Registry, points: &[(String, Namespace, (f64, f64))]) {
    for (code, namespace, (lon, lat)) in points {
        let carries_point = *namespace != Namespace::Continent;
        let found = reg
            .continents
            .iter_mut()
            .chain(reg.countries.iter_mut())
            .chain(reg.provinces.iter_mut())
            .find(|e| &e.code == code);
        match found {
            Some(e) => {
                if carries_point {
                    e.point = Some(round_point([*lon, *lat]));
                }
            }
            None => {
                let id = reg.next_id();
                let entry = Entry {
                    code: code.clone(),
                    id,
                    point: carries_point.then(|| round_point([*lon, *lat])),
                };
                match namespace {
                    Namespace::Continent => reg.continents.push(entry),
                    Namespace::Country => reg.countries.push(entry),
                    Namespace::Province => reg.provinces.push(entry),
                }
            }
        }
    }
}

pub fn to_toml_sorted(reg: &Registry) -> Result<String> {
    let sorted = |list: &[Entry]| {
        let mut v = list.to_vec();
        v.sort_by(|a, b| a.code.cmp(&b.code));
        v
    };
    let out = Registry {
        schema: reg.schema,
        continents: sorted(&reg.continents),
        countries: sorted(&reg.countries),
        provinces: sorted(&reg.provinces),
    };
    toml::to_string(&out).context("serializing registry")
}
