//! Build the deterministic entity list (continents + countries collapsed
//! by ADM0_A3) from parsed Natural Earth features.
//!
//! TODO: ids come from the frozen registry (the union across all
//! worldview files), so this stays unchanged for Phase 2 base+delta.

use std::collections::BTreeMap;

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

/// Map a Natural Earth `CONTINENT` value to the MVP's two-letter continent code.
fn continent_code(continent: &str) -> &'static str {
    match continent {
        "Africa" => "AF",
        "Antarctica" => "AN",
        "Asia" => "AS",
        "Europe" => "EU",
        "North America" => "NA",
        "Oceania" => "OC",
        "South America" => "SA",
        // Should already be filtered out by parse, but be defensive.
        other => panic!("unexpected CONTINENT value: {other}"),
    }
}

/// Public wrapper so `registry_gen` can derive a feature's continent code.
pub fn continent_code_pub(continent: &str) -> &'static str {
    continent_code(continent)
}

pub fn assemble_entities(features: &[ParsedFeature], registry: &Registry) -> Result<EntityModel> {
    // Group features by ADM0_A3 (collapse step), BTreeMap for deterministic
    // iteration. NOTE: iteration order no longer determines IDs — the
    // registry does — but determinism still matters for area/raster passes.
    let mut groups: BTreeMap<String, Vec<&ParsedFeature>> = BTreeMap::new();
    for f in features {
        groups.entry(f.adm0_a3.clone()).or_default().push(f);
    }

    // Continents present among metropoles (first feature per ADM0_A3).
    let mut continent_codes: BTreeMap<&'static str, ()> = BTreeMap::new();
    for group in groups.values() {
        continent_codes.insert(continent_code(&group[0].continent), ());
    }

    let mut entities: Vec<GeoEntity> = Vec::new();
    let mut continent_id_for_code: BTreeMap<&'static str, GeoEntityId> = BTreeMap::new();
    for code in continent_codes.keys() {
        let id = registry.id_for_continent(code)?; // CI gate 1 (continents)
        continent_id_for_code.insert(code, id);
        entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Continent,
            iso_code: code.to_string(),
            name_key: format!("continent.{code}.name"),
            parent_id: None,
            total_area_m2: 0,
        });
    }

    let mut geometry_for_country: BTreeMap<String, MultiPolygon<f64>> = BTreeMap::new();
    for (adm0, group) in groups.iter() {
        let id = registry.id_for_country(adm0)?; // CI gate 1 (countries)
        let parent_code = continent_code(&group[0].continent);
        let parent_id = continent_id_for_code
            .get(parent_code)
            .copied()
            .ok_or_else(|| anyhow!("continent {parent_code} unexpectedly missing for {adm0}"))?;
        entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Country,
            iso_code: adm0.clone(),
            name_key: format!("country.{adm0}.name"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Entry, Registry};
    use geo_types::{Coord, LineString, Polygon};

    fn feat(adm0: &str, continent: &str) -> ParsedFeature {
        let sq = Polygon::new(
            LineString(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 0.0, y: 0.0 },
            ]),
            vec![],
        );
        ParsedFeature {
            adm0_a3: adm0.into(),
            iso_a3: adm0.into(),
            name: adm0.into(),
            continent: continent.into(),
            geometry: MultiPolygon(vec![sq]),
        }
    }

    fn reg() -> Registry {
        Registry {
            schema: 1,
            continents: vec![Entry {
                code: "AS".into(),
                id: 5,
                refs: Default::default(),
            }],
            countries: vec![Entry {
                code: "AAA".into(),
                id: 3,
                refs: Default::default(),
            }],
        }
    }

    #[test]
    fn ids_come_from_registry_not_position() {
        let m = assemble_entities(&[feat("AAA", "Asia")], &reg()).unwrap();
        let aaa = m.entities.iter().find(|e| e.iso_code == "AAA").unwrap();
        assert_eq!(aaa.id, GeoEntityId(3));
        assert_eq!(aaa.parent_id, Some(GeoEntityId(5)));
    }

    #[test]
    fn unknown_adm0_is_an_error() {
        let err = assemble_entities(&[feat("ZZZ", "Asia")], &reg())
            .unwrap_err()
            .to_string();
        assert!(err.contains("ZZZ"), "got: {err}");
    }
}
