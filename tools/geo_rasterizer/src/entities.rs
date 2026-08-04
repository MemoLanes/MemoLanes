//! Build the deterministic entity list from parsed Natural Earth features, and
//! decide which country each province belongs to.
//!
//! TODO: ids come from the frozen registry (the union across all
//! worldview files), so this stays unchanged for Phase 2 base+delta.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Result};
use geo_data_format::{GeoEntity, GeoEntityId, GeoEntityKind, Worldview};
use geo_types::MultiPolygon;

use crate::admin0::Admin0Feature;
use crate::admin1::Admin1Feature;
use crate::refine::{block_majority, Coverage};
use crate::registry::Registry;

/// All the entity-level outputs the rasterizer needs.
#[derive(Debug)]
pub struct EntityModel {
    /// Entities sorted by id (ascending). Kind order follows the registry's id
    /// allocation, not structural position — do not rely on continents preceding countries.
    pub entities: Vec<GeoEntity>,
    /// `ADM0_A3 → merged MultiPolygon` for each country (ready for rasterization).
    pub geometry_for_country: BTreeMap<String, MultiPolygon<f64>>,
    /// `adm1_code → MultiPolygon` for each province of this worldview
    pub geometry_for_province: BTreeMap<String, MultiPolygon<f64>>,
    /// `adm1_code → GeoEntityId`, resolved from the frozen registry
    pub province_ids: BTreeMap<String, GeoEntityId>,
    /// `adm1_code → ADM0_A3` exactly as Natural Earth's admin-1 file declares it
    pub province_declared_adm0: BTreeMap<String, String>,
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

pub fn sovereign_member<'a>(group: &[&'a Admin0Feature]) -> Option<&'a Admin0Feature> {
    match group {
        [only] => Some(only),
        members => members
            .iter()
            .copied()
            .find(|f| f.feature_type == "Country"),
    }
}

pub fn group_continent_code(group: &[&Admin0Feature]) -> Result<&'static str> {
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
    groups: impl IntoIterator<Item = &'g Vec<&'f Admin0Feature>>,
) -> Result<BTreeSet<&'static str>>
where
    'f: 'g,
{
    groups
        .into_iter()
        .map(|g| group_continent_code(g))
        .collect()
}

pub fn assemble_entities(features: &[Admin0Feature], registry: &Registry) -> Result<EntityModel> {
    // Group features by ADM0_A3 (collapse step), BTreeMap for deterministic
    // iteration. NOTE: iteration order no longer determines IDs — the
    // registry does — but determinism still matters for area/raster passes.
    let mut groups: BTreeMap<String, Vec<&Admin0Feature>> = BTreeMap::new();
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
        geometry_for_province: BTreeMap::new(),
        province_ids: BTreeMap::new(),
        province_declared_adm0: BTreeMap::new(),
    })
}

pub fn collect_provinces(
    model: &mut EntityModel,
    features: &[Admin1Feature],
    registry: &Registry,
) -> Result<()> {
    for f in features {
        let id = registry.id_for_province(&f.adm1_code)?; // CI gate 1 (provinces)
        model.province_ids.insert(f.adm1_code.clone(), id);
        model
            .province_declared_adm0
            .insert(f.adm1_code.clone(), f.adm0_a3.clone());
        model
            .geometry_for_province
            .entry(f.adm1_code.clone())
            .or_insert_with(|| MultiPolygon(Vec::new()))
            .0
            .extend(f.geometry.0.iter().cloned());
    }
    Ok(())
}

/// `(worldview id, adm1_code)` for provinces this worldview's admin-0 leaves no
/// land for, so [`resolve_province_parents`] drops them instead of failing.
const UNPARENTED_PROVINCES: &[(&str, &str)] = &[
    ("iso", "ASM-5001"),  // Swains Island
    ("iso", "GUY-675"),   // Barima-Waini (Essequibo)
    ("iso", "GUY-680"),   // Cuyuni-Mazaruni (Essequibo)
    ("iso", "IND-20012"), // Ladakh (disputed Kashmir)
    ("iso", "PFA+00?"),   // Paracel Islands
    ("iso", "PGA+00?"),   // Spratly Islands
    ("iso", "RUS-283"),   // Autonomous Republic of Crimea
    ("iso", "RUS-5482"),  // Sevastopol
    ("iso", "UMI-5179"),  // Wake Island
    ("usa", "PFA+00?"),   // Paracel Islands
];

/// Resolved according country's geo definition rather than provinces's adm0
const MASK_OVERRULES_DECLARED_PARENT: &[(&str, &str, &str)] = &[
    ("chn", "RUS-283", "UKR"),
    ("chn", "RUS-5482", "UKR"),
    ("usa", "GEO-3015", "B35"),
    ("usa", "RUS-283", "UKR"),
    ("usa", "RUS-5482", "UKR"),
];

pub fn resolve_province_parents(
    model: &EntityModel,
    tally: &Coverage,
    worldview: Worldview,
) -> Result<BTreeMap<GeoEntityId, GeoEntityId>> {
    let wv = worldview.spec().id;
    let mut id_of_country: BTreeMap<&str, GeoEntityId> = BTreeMap::new();
    let mut code_of_country: BTreeMap<GeoEntityId, &str> = BTreeMap::new();
    for entity in &model.entities {
        if matches!(entity.kind, GeoEntityKind::Country) {
            id_of_country.insert(entity.canonical_code.as_str(), entity.id);
            code_of_country.insert(entity.id, entity.canonical_code.as_str());
        }
    }
    let overruled: BTreeMap<&'static str, &'static str> = MASK_OVERRULES_DECLARED_PARENT
        .iter()
        .filter(|(w, _, _)| *w == wv)
        .map(|(_, code, mask)| (*code, *mask))
        .collect();
    let landless_allowed: BTreeSet<&'static str> = UNPARENTED_PROVINCES
        .iter()
        .filter(|(w, _)| *w == wv)
        .map(|(_, code)| *code)
        .collect();

    let mut parents: BTreeMap<GeoEntityId, GeoEntityId> = BTreeMap::new();
    let mut dropped: BTreeSet<&'static str> = BTreeSet::new();
    // Every offender is collected and reported in one run: an NE pin bump
    // produces these in bulk, and each rediscovery costs a full rasterize.
    let mut landless: Vec<String> = Vec::new();
    let mut stale_overrules: Vec<String> = Vec::new();

    for (adm1_code, &id) in &model.province_ids {
        let by_country = tally.get(id.0 as usize);
        let majority = by_country.and_then(block_majority);
        let declared_code = model
            .province_declared_adm0
            .get(adm1_code)
            .map(String::as_str)
            .ok_or_else(|| {
                anyhow!(
                    "province `{adm1_code}` has no declared ADM0_A3 — collect_provinces must \
                         record one for every province it registers"
                )
            })?;
        let declared = id_of_country.get(declared_code).copied();

        let parent = match overruled.get(adm1_code.as_str()) {
            Some(&mask_code) => match (declared, majority) {
                (Some(d), Some(m)) if d != m && code_of_country.get(&m) == Some(&mask_code) => {
                    Some(m)
                }
                _ => {
                    stale_overrules.push(format!(
                        "`{adm1_code}` (declares `{declared_code}`, mask majority {}, entry says \
                         `{mask_code}`)",
                        majority
                            .and_then(|m| code_of_country.get(&m).copied())
                            .unwrap_or("none"),
                    ));
                    continue;
                }
            },
            None => declared.or(majority),
        };

        let covered = parent
            .and_then(|p| by_country.and_then(|m| m.get(&p).copied()))
            .unwrap_or(0);
        if covered == 0 {
            if let Some(listed) = landless_allowed.get(adm1_code.as_str()) {
                dropped.insert(listed);
                continue;
            }
            landless.push(format!(
                "`{adm1_code}` (declares `{declared_code}`, {}; mask majority {})",
                match declared {
                    Some(_) => "a country here",
                    None => "not a country here",
                },
                majority
                    .and_then(|m| code_of_country.get(&m).copied())
                    .unwrap_or("none"),
            ));
            continue;
        }
        parents.insert(id, parent.expect("a nonzero block count implies a parent"));
    }

    let mut problems: Vec<String> = Vec::new();
    if !landless.is_empty() {
        problems.push(format!(
            "{} province(s) have no block under the country they would be parented to:\n    {}\n  \
             Either this worldview's admin-0 leaves that land unclaimed, in which case add \
             (\"{wv}\", \"<adm1_code>\") to UNPARENTED_PROVINCES with a note, or its admin-0 \
             deliberately places the province in the country named above, which belongs in \
             MASK_OVERRULES_DECLARED_PARENT",
            landless.len(),
            landless.join("\n    "),
        ));
    }
    if !stale_overrules.is_empty() {
        problems.push(format!(
            "MASK_OVERRULES_DECLARED_PARENT has {} entr{} the mask no longer supports:\n    {}\n  \
             Drop them so each province falls back to its default parent — the declared code \
             where that is a country here, otherwise the block majority",
            stale_overrules.len(),
            if stale_overrules.len() == 1 {
                "y"
            } else {
                "ies"
            },
            stale_overrules.join("\n    "),
        ));
    }
    let stale_skips: Vec<&str> = landless_allowed
        .iter()
        .copied()
        .filter(|code| !dropped.contains(code) && model.province_ids.contains_key(*code))
        .collect();
    if !stale_skips.is_empty() {
        problems.push(format!(
            "UNPARENTED_PROVINCES lists {} which the country mask now gives land to — drop the \
             stale entr{} so the province ships",
            stale_skips.join(", "),
            if stale_skips.len() == 1 { "y" } else { "ies" },
        ));
    }
    if !problems.is_empty() {
        bail!(
            "province parenting failed in worldview `{wv}`:\n  {}",
            problems.join("\n  ")
        );
    }
    Ok(parents)
}

pub fn attach_province_entities(
    model: &mut EntityModel,
    parent_of: &BTreeMap<GeoEntityId, GeoEntityId>,
) {
    for (adm1_code, &id) in &model.province_ids {
        let Some(&parent_id) = parent_of.get(&id) else {
            continue;
        };
        model.entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Province,
            canonical_code: adm1_code.clone(),
            iso_a3_eh: None,
            name_key: format!("province.{adm1_code}"),
            parent_id: Some(parent_id),
            total_area_m2: 0,
        });
    }
    model.entities.sort_by_key(|e| e.id.0);
}
