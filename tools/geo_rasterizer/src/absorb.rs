use anyhow::{bail, Result};
use geo_types::MultiPolygon;

use crate::admin0::Admin0Feature;
use crate::policy::{self, AbsorbMode};

pub(crate) fn apply_absorptions(
    features: &mut Vec<Admin0Feature>,
    worldview: &str,
) -> Result<Vec<(String, MultiPolygon<f64>)>> {
    let policy = policy::get()?;
    let mut absorbed: Vec<(String, MultiPolygon<f64>)> = Vec::new();
    let mut attributions: Vec<(String, MultiPolygon<f64>)> = Vec::new();
    let mut retained = Vec::with_capacity(features.len());
    for mut f in features.drain(..) {
        match policy
            .absorb
            .iter()
            .find(|rule| rule.worldview == worldview && rule.code == f.adm0_a3)
        {
            Some(rule) if rule.mode == AbsorbMode::Merge => {
                if f.feature_type == "Country" {
                    bail!(
                        "geo_policy.toml merges `{}` but its TYPE is \"Country\" — a \
                         country-typed feature should be demoted, not erased; fix the absorb mode",
                        f.adm0_a3
                    );
                }
                if let Some(target) = &rule.attribute_to {
                    attributions.push((target.clone(), f.geometry.clone()));
                }
                absorbed.push((rule.into.clone(), f.geometry));
            }
            Some(rule) => {
                absorbed.push((rule.into.clone(), f.geometry.clone()));
                f.demoted_into = Some(rule.into.clone());
                retained.push(f);
            }
            None => retained.push(f),
        }
    }
    *features = retained;

    for (into, geometry) in absorbed {
        match features.iter_mut().find(|f| f.adm0_a3 == into) {
            Some(sovereign) => sovereign.geometry.0.extend(geometry.0),
            None => bail!(
                "absorption target `{into}` not found in worldview `{worldview}` \
                 (its dependency's geometry would be lost)"
            ),
        }
    }
    Ok(attributions)
}
