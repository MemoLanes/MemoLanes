//! Parse Natural Earth admin-1 (states/provinces) GeoJSON.
//!
//! Two filters run here, both by design (see the provinces design spec §4):
//!
//! 1. `<ADM0>+99?` features are Natural Earth's unassigned-remainder sentinel —
//!    the part of a country belonging to no admin-1 unit. They are exactly the
//!    features with no name in any of NE's 25 language columns. Dropping them
//!    lets that land fall back to the country leaf, which is what it means.
//! 2. A non-null value in the worldview's `FCLASS_*` column means that point of
//!    view does not treat the feature as an admin-1 unit.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use geo_data_format::Worldview;
use geo_types::{Geometry, MultiPolygon};

pub struct Admin1Feature {
    /// `adm1_code` (e.g. `MYS-1186`). Natural Earth's unique admin-1 key and
    /// this entity's `canonical_code`.
    pub adm1_code: String,
    /// `adm0_a3` of the country this feature is drawn under. The primary
    /// source for this province's parent whenever it names a country in the
    /// worldview being built
    pub adm0_a3: String,
    /// `iso_3166_2`. The CLDR subdivision join key
    pub iso_3166_2: String,
    pub name_en: Option<String>,
    pub name_zh: Option<String>,
    pub geometry: MultiPolygon<f64>,
}

pub fn is_unassigned_remainder(adm1_code: &str) -> bool {
    adm1_code.ends_with("+99?")
}

pub fn parse_admin1(path: &Path, worldview: Worldview) -> Result<Vec<Admin1Feature>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading admin-1 geojson at {}", path.display()))?;
    let collection: geojson::FeatureCollection = serde_json::from_str(&raw)
        .with_context(|| format!("parsing admin-1 geojson at {}", path.display()))?;

    let fclass = worldview.spec().admin1_fclass_field;
    let mut out = Vec::with_capacity(collection.features.len());
    for (idx, feature) in collection.features.into_iter().enumerate() {
        let props = feature
            .properties
            .as_ref()
            .ok_or_else(|| anyhow!("admin-1 feature {idx}: missing properties"))?;
        let string = |key: &str| props.get(key).and_then(|v| v.as_str());

        let adm1_code = string("adm1_code")
            .ok_or_else(|| anyhow!("admin-1 feature {idx}: missing adm1_code"))?
            .to_string();
        if is_unassigned_remainder(&adm1_code) {
            continue;
        }
        if string(fclass).is_some() {
            continue;
        }

        let adm0_a3 = string("adm0_a3")
            .ok_or_else(|| anyhow!("admin-1 feature {idx} ({adm1_code}): missing adm0_a3"))?
            .to_string();
        let iso_3166_2 = string("iso_3166_2")
            .ok_or_else(|| anyhow!("admin-1 feature {idx} ({adm1_code}): missing iso_3166_2"))?
            .to_string();

        let geom_value = feature
            .geometry
            .ok_or_else(|| anyhow!("admin-1 feature {idx} ({adm1_code}): missing geometry"))?;
        let geom: Geometry<f64> = (&geom_value)
            .try_into()
            .with_context(|| format!("admin-1 feature {idx} ({adm1_code}): invalid geometry"))?;
        let geometry: MultiPolygon<f64> = match geom {
            Geometry::Polygon(p) => MultiPolygon(vec![p]),
            Geometry::MultiPolygon(mp) => mp,
            _ => bail!("admin-1 feature {idx} ({adm1_code}): expected Polygon/MultiPolygon"),
        };

        out.push(Admin1Feature {
            adm1_code,
            adm0_a3,
            iso_3166_2,
            name_en: string("name_en").map(str::to_string),
            name_zh: string("name_zh").map(str::to_string),
            geometry,
        });
    }
    Ok(out)
}
