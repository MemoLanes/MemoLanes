//! The admin-2 (prefecture-level) tier, read from the committed, zstd-compressed
//! CHN snapshot that `tools/geo_admin2_snapshot` slices out of Overture Maps.
//! This module enforces
//! the property contract that tool must satisfy: `adcode` (string, 4 or 6
//! digits), `name_zh` and `name_en`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use geo_types::{Geometry, MultiPolygon};
use serde::Deserialize;

/// One admin-2 feature, before registry-id assignment and rasterization.
pub struct Admin2Feature {
    /// This tier's stable code, e.g. `CN-4403`. The registry join key.
    pub canonical_code: String,
    /// `adm1_code` of the containing province — this tier's parent join key.
    pub parent_adm1: String,
    pub name_en: String,
    pub name_zh: String,
    pub geometry: MultiPolygon<f64>,
}

/// Directory holding the snapshot and its province map, each under a fixed
/// name. Overridable as a unit (`--admin2-dir`) so a fixture world can supply
/// both together.
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("admin2")
}

/// The files [`load`] reads — this tier's contribution to the bin provenance
/// hash.
pub fn provenance_inputs(dir: &Path) -> Vec<PathBuf> {
    vec![snapshot_path(dir), province_map_path(dir)]
}

pub fn load(dir: &Path) -> Result<Vec<Admin2Feature>> {
    let province_map = load_province_map(&province_map_path(dir))?;
    parse_snapshot(&snapshot_path(dir), &province_map)
}

fn snapshot_path(dir: &Path) -> PathBuf {
    dir.join("chn_admin2.geojson.zst")
}

fn province_map_path(dir: &Path) -> PathBuf {
    dir.join("chn_province_map.toml")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvinceMapDoc {
    #[allow(dead_code)]
    schema: u32,
    province: Vec<ProvinceMapRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvinceMapRow {
    prefix: String,
    adm1_code: String,
}

/// Adcode `prefix → adm1_code`, bailing on a duplicate prefix.
fn load_province_map(path: &Path) -> Result<BTreeMap<String, String>> {
    let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let doc: ProvinceMapDoc =
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    let mut map = BTreeMap::new();
    for row in doc.province {
        if let Some(prev) = map.insert(row.prefix.clone(), row.adm1_code.clone()) {
            bail!(
                "{}: prefix `{}` is mapped to both `{prev}` and `{}`",
                path.display(),
                row.prefix,
                row.adm1_code
            );
        }
    }
    Ok(map)
}

/// Each feature's province is selected from `province_map` by the adcode's
/// first two digits (GB/T 2260's province prefix).
fn parse_snapshot(
    path: &Path,
    province_map: &BTreeMap<String, String>,
) -> Result<Vec<Admin2Feature>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("opening CHN admin-2 snapshot at {}", path.display()))?;
    let raw = zstd::decode_all(file)
        .with_context(|| format!("decompressing CHN admin-2 snapshot at {}", path.display()))?;
    let collection: geojson::FeatureCollection = serde_json::from_slice(&raw)
        .with_context(|| format!("parsing CHN admin-2 snapshot at {}", path.display()))?;

    let mut out = Vec::with_capacity(collection.features.len());
    let mut seen_adcodes = BTreeSet::new();
    for (idx, feature) in collection.features.into_iter().enumerate() {
        let props = feature
            .properties
            .as_ref()
            .ok_or_else(|| anyhow!("CHN admin-2 feature {idx}: missing properties"))?;
        let string = |key: &str| props.get(key).and_then(|v| v.as_str());

        let adcode = string("adcode")
            .ok_or_else(|| anyhow!("CHN admin-2 feature {idx}: missing adcode"))?
            .to_string();
        if !seen_adcodes.insert(adcode.clone()) {
            bail!("CHN admin-2 feature {idx}: duplicate adcode `{adcode}`");
        }

        let name_zh = string("name_zh")
            .ok_or_else(|| anyhow!("CHN admin-2 feature {idx} (adcode {adcode}): missing name_zh"))?
            .to_string();
        let name_en = string("name_en")
            .ok_or_else(|| anyhow!("CHN admin-2 feature {idx} (adcode {adcode}): missing name_en"))?
            .to_string();

        if adcode.len() < 2 {
            bail!(
                "CHN admin-2 feature {idx}: adcode `{adcode}` is shorter than a 2-digit province prefix"
            );
        }
        let prefix = &adcode[..2];
        let parent_adm1 = province_map.get(prefix).cloned().ok_or_else(|| {
            anyhow!(
                "CHN admin-2 feature {idx} (adcode {adcode}): no province mapped for prefix `{prefix}`"
            )
        })?;

        let geom_value = feature.geometry.ok_or_else(|| {
            anyhow!("CHN admin-2 feature {idx} (adcode {adcode}): missing geometry")
        })?;
        let geom: Geometry<f64> = (&geom_value).try_into().with_context(|| {
            format!("CHN admin-2 feature {idx} (adcode {adcode}): invalid geometry")
        })?;
        let geometry: MultiPolygon<f64> = match geom {
            Geometry::Polygon(p) => MultiPolygon(vec![p]),
            Geometry::MultiPolygon(mp) => mp,
            _ => {
                bail!("CHN admin-2 feature {idx} (adcode {adcode}): expected Polygon/MultiPolygon")
            }
        };

        out.push(Admin2Feature {
            canonical_code: format!("CN-{adcode}"),
            parent_adm1,
            name_en,
            name_zh,
            geometry,
        });
    }
    Ok(out)
}
