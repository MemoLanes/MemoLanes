use anyhow::{bail, Result};

use crate::parse::ParsedFeature;

/// `(worldview id, absorbed ADM0_A3, parent ADM0_A3)`.
const ABSORPTIONS: &[(&str, &str, &str)] = &[
    ("chn", "HKG", "CHN"),
    ("chn", "MAC", "CHN"),
    ("chn", "SCR", "CHN"),
    // PGA is already absorbed into CHN
];

pub(crate) fn apply_absorptions(features: &mut Vec<ParsedFeature>, worldview: &str) -> Result<()> {
    let mut absorbed: Vec<(&'static str, geo_types::MultiPolygon<f64>)> = Vec::new();
    features.retain(|f| {
        match ABSORPTIONS
            .iter()
            .find(|(wv, from, _)| *wv == worldview && *from == f.adm0_a3)
        {
            Some((_, _, into)) => {
                absorbed.push((into, f.geometry.clone()));
                false
            }
            None => true,
        }
    });

    for (into, geometry) in absorbed {
        match features.iter_mut().find(|f| f.adm0_a3 == into) {
            Some(sovereign) => sovereign.geometry.0.extend(geometry.0),
            None => bail!(
                "absorption target `{into}` not found in worldview `{worldview}` \
                 (its dependency's geometry would be lost)"
            ),
        }
    }
    Ok(())
}
