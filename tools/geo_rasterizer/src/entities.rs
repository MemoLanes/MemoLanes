//! Build the deterministic entity list (continents + countries collapsed
//! by ADM0_A3) from parsed Natural Earth features.
//!
//! TODO: ids come from the frozen registry (the union across all
//! worldview files), so this stays unchanged for Phase 2 base+delta.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use geo_data_format::{GeoEntity, GeoEntityId, GeoEntityKind};
use geo_types::MultiPolygon;

use crate::parse::ParsedFeature;
use crate::registry::Registry;

/// All the entity-level outputs the rasterizer needs.
#[derive(Debug)]
pub struct EntityModel {
    /// Entities sorted by id (ascending). Kind order follows the registry's id
    /// allocation, not structural position — do not rely on continents preceding countries.
    pub entities: Vec<GeoEntity>,
    /// `ADM0_A3 → merged MultiPolygon` for each country (ready for rasterization).
    pub geometry_for_country: BTreeMap<String, MultiPolygon<f64>>,
}

pub fn feature_continent_code(continent: &str, region_un: &str) -> &'static str {
    match continent {
        "Africa" => "AF",
        "Antarctica" => "AN",
        "Asia" => "AS",
        "Europe" => "EU",
        "North America" => "NA",
        "Oceania" => "OC",
        "South America" => "SA",
        "Seven seas (open ocean)" => region_un_code(region_un),
        other => panic!("unexpected CONTINENT value: {other}"),
    }
}

/// Map a UN M49 `REGION_UN` value to a continent code. Used only as the
/// "Seven seas" fallback. `Americas` → `SA` (South America) by convention.
fn region_un_code(region_un: &str) -> &'static str {
    match region_un {
        "Africa" => "AF",
        "Antarctica" => "AN",
        "Asia" => "AS",
        "Europe" => "EU",
        "Oceania" => "OC",
        "Americas" => "SA",
        other => panic!("unexpected REGION_UN value for Seven-seas feature: {other}"),
    }
}

pub fn sovereign_member<'a>(group: &[&'a ParsedFeature]) -> Option<&'a ParsedFeature> {
    match group {
        [only] => Some(only),
        members => members
            .iter()
            .copied()
            .find(|f| f.feature_type == "Country"),
    }
}

pub fn group_continent_code(group: &[&ParsedFeature]) -> Result<&'static str> {
    if let Some(sovereign) = sovereign_member(group) {
        return Ok(feature_continent_code(
            &sovereign.continent,
            &sovereign.region_un,
        ));
    }
    let codes: BTreeSet<&'static str> = group
        .iter()
        .map(|f| feature_continent_code(&f.continent, &f.region_un))
        .collect();
    match codes.len() {
        1 => Ok(codes.into_iter().next().expect("checked non-empty")),
        _ => Err(anyhow!(
            "`{}` has no sovereign member (no `TYPE == \"Country\"`) and its members disagree on \
             continent ({:?}) — nothing can decide this from the data, so pin the continent for \
             this code rather than letting Natural Earth's row order pick",
            group[0].adm0_a3,
            codes,
        )),
    }
}

fn continent_set<'g, 'f>(
    groups: impl IntoIterator<Item = &'g Vec<&'f ParsedFeature>>,
) -> Result<BTreeSet<&'static str>>
where
    'f: 'g,
{
    groups
        .into_iter()
        .map(|g| group_continent_code(g))
        .collect()
}

pub fn assemble_entities(features: &[ParsedFeature], registry: &Registry) -> Result<EntityModel> {
    // Group features by ADM0_A3 (collapse step), BTreeMap for deterministic
    // iteration. NOTE: iteration order no longer determines IDs — the
    // registry does — but determinism still matters for area/raster passes.
    let mut groups: BTreeMap<String, Vec<&ParsedFeature>> = BTreeMap::new();
    for f in features {
        groups.entry(f.adm0_a3.clone()).or_default().push(f);
    }

    let mut entities: Vec<GeoEntity> = Vec::new();
    let mut continent_id_for_code: BTreeMap<&'static str, GeoEntityId> = BTreeMap::new();
    for code in continent_set(groups.values())? {
        let id = registry.id_for_continent(code)?; // CI gate 1 (continents)
        continent_id_for_code.insert(code, id);
        entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Continent,
            canonical_code: code.to_string(),
            iso_a3_eh: None,
            name_key: format!("continent.{code}"),
            parent_id: None,
            total_area_m2: 0,
        });
    }

    let mut geometry_for_country: BTreeMap<String, MultiPolygon<f64>> = BTreeMap::new();
    for (adm0, group) in groups.iter() {
        let id = registry.id_for_country(adm0)?; // CI gate 1 (countries)
        let parent_code = group_continent_code(group)?;
        let parent_id = continent_id_for_code
            .get(parent_code)
            .copied()
            .ok_or_else(|| anyhow!("continent {parent_code} unexpectedly missing for {adm0}"))?;
        // The entity's ISO code is the sovereign's. A single-feature group *is*
        // the whole country, so its own `ISO_A3_EH` is authoritative (this is
        // the sovereign even when NE's `ADM0_A3` is a non-ISO code, e.g.
        // Palestine PSX→PSE, S. Sudan SDS→SSD). A collapsed group (only the ISO
        // worldview: France, Norway, Netherlands, New Zealand, and the Cocos +
        // Christmas `IOA` bucket) bundles detached dependencies under one key;
        // there the mainland is the sole `TYPE == "Country"` member, and the
        // dependencies (`Geo unit`/`Dependency`/...) must not shadow it. `None`
        // when no sovereign member exists (`IOA`) or the code is NE's `-99`
        // sentinel.
        let iso_a3_eh = match group.as_slice() {
            [only] => Some(only.iso_a3_eh.clone()),
            members => members
                .iter()
                .find(|f| f.feature_type == "Country")
                .map(|f| f.iso_a3_eh.clone()),
        }
        .filter(|code| code != "-99");
        entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Country,
            canonical_code: adm0.clone(),
            iso_a3_eh,
            name_key: format!("country.{adm0}"),
            parent_id: Some(parent_id),
            total_area_m2: 0,
        });

        let mut merged: Vec<geo_types::Polygon<f64>> = Vec::new();
        for f in group {
            for poly in &f.geometry.0 {
                merged.push(poly.clone());
            }
        }
        geometry_for_country.insert(adm0.clone(), MultiPolygon(merged));
    }

    // Sort by id for deterministic serialization order.
    entities.sort_by_key(|e| e.id.0);

    Ok(EntityModel {
        entities,
        geometry_for_country,
    })
}
