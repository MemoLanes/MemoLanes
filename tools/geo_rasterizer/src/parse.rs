//! Parse Natural Earth admin-0 GeoJSON.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use geo_types::{Geometry, MultiPolygon};

/// One Natural Earth feature, post-filter, with the fields the rasterizer needs.
pub struct ParsedFeature {
    /// `ADM0_A3` (NE's canonical key). Always present after parsing.
    pub adm0_a3: String,
    /// `ISO_A3`. May be `'-99'` (NE's missing-value sentinel).
    pub iso_a3: String,
    /// `NAME` (English). For logging/diagnostics.
    pub name: String,
    /// `CONTINENT`.
    pub continent: String,
    /// Geometry as `MultiPolygon` (Polygons are wrapped to a 1-element MP for uniformity).
    pub geometry: MultiPolygon<f64>,
}

/// Parse a Natural Earth admin-0 countries GeoJSON. Filters out features
/// whose `CONTINENT == "Seven seas (open ocean)"`. Returns the rest.
pub fn parse_geojson(path: &Path) -> Result<Vec<ParsedFeature>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading geojson at {}", path.display()))?;
    let collection: geojson::FeatureCollection = serde_json::from_str(&raw)
        .with_context(|| format!("parsing geojson at {}", path.display()))?;

    let mut out = Vec::with_capacity(collection.features.len());
    for (idx, feature) in collection.features.into_iter().enumerate() {
        let props = feature
            .properties
            .as_ref()
            .ok_or_else(|| anyhow!("feature {idx}: missing properties"))?;
        let adm0_a3 = props
            .get("ADM0_A3")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("feature {idx}: missing ADM0_A3"))?
            .to_string();
        let iso_a3 = props
            .get("ISO_A3")
            .and_then(|v| v.as_str())
            .unwrap_or("-99")
            .to_string();
        let name = props
            .get("NAME")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();
        let continent = props
            .get("CONTINENT")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("feature {idx} ({adm0_a3}): missing CONTINENT"))?
            .to_string();
        if continent == "Seven seas (open ocean)" {
            continue;
        }
        let geom_value = feature
            .geometry
            .ok_or_else(|| anyhow!("feature {idx} ({adm0_a3}): missing geometry"))?;
        let geom: Geometry<f64> = (&geom_value)
            .try_into()
            .with_context(|| format!("feature {idx} ({adm0_a3}): invalid geometry"))?;
        let mp: MultiPolygon<f64> = match geom {
            Geometry::Polygon(p) => MultiPolygon(vec![p]),
            Geometry::MultiPolygon(mp) => mp,
            _ => bail!("feature {idx} ({adm0_a3}): expected Polygon/MultiPolygon"),
        };
        out.push(ParsedFeature {
            adm0_a3,
            iso_a3,
            name,
            continent,
            geometry: mp,
        });
    }
    Ok(out)
}

/// Verify that no ring crosses the antimeridian via a single edge between
/// two non-polar vertices. Natural Earth pre-splits polygons that cross
/// ±180; if an edge connects two vertices with |Δlng| > 180° at non-polar
/// latitudes, that assumption was violated.
///
/// Polar caps (e.g., Antarctica) legitimately have edges whose longitudes
/// differ by ~360° because the ring encloses a pole. Such edges are
/// allowed when both endpoints have |lat| > POLAR_LAT_THRESHOLD.
pub fn validate_no_antimeridian_span(features: &[ParsedFeature]) -> Result<()> {
    const POLAR_LAT_THRESHOLD: f64 = 85.0;
    for f in features {
        for poly in &f.geometry.0 {
            check_edges(&f.adm0_a3, poly.exterior(), POLAR_LAT_THRESHOLD)?;
            for hole in poly.interiors() {
                check_edges(&f.adm0_a3, hole, POLAR_LAT_THRESHOLD)?;
            }
        }
    }
    Ok(())
}

fn check_edges(
    adm0: &str,
    ring: &geo_types::LineString<f64>,
    polar_threshold_deg: f64,
) -> Result<()> {
    let pts: Vec<&geo_types::Coord<f64>> = ring.0.iter().collect();
    if pts.len() < 2 {
        return Ok(());
    }
    for window in pts.windows(2) {
        let (a, b) = (window[0], window[1]);
        let dlng = (b.x - a.x).abs();
        if dlng > 180.0 {
            // Allow if both endpoints are very close to a pole — the ring
            // is closing across a polar cap, not crossing the antimeridian.
            let both_polar = a.y.abs() >= polar_threshold_deg && b.y.abs() >= polar_threshold_deg;
            if !both_polar {
                bail!(
                    "feature {adm0}: edge spans {:.2}° longitude between non-polar \
                     vertices ({}, {}) → ({}, {}); antimeridian split assumption broken",
                    dlng,
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
    }
    Ok(())
}
