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
use crate::policy;
use crate::refine::{block_majority, Coverage};
use crate::registry::Registry;

#[derive(Debug)]
pub struct DemotedProvince {
    pub iso_a3_eh: Option<String>,
}

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
    /// Territories this worldview demotes
    pub demoted: BTreeMap<String, DemotedProvince>,
}

pub struct MergedSource {
    pub code: String,
    pub target: String,
    pub geometry: MultiPolygon<f64>,
}

pub struct Admin1Policy {
    pub features: Vec<Admin1Feature>,
    pub merged_sources: Vec<MergedSource>,
}

pub fn apply_admin1_policy(
    features: Vec<Admin1Feature>,
    worldview: Worldview,
    admin0: &[Admin0Feature],
) -> Result<Admin1Policy> {
    let policy = policy::get()?;
    let kept: Vec<Admin1Feature> = features
        .into_iter()
        .filter(|f| !policy.drop_admin1_in.contains(&f.adm0_a3))
        .collect();

    // A territory's sole admin-1 unit duplicates its own parent when the
    // territory is already an entity here: Natural Earth draws these
    // whole-territory rows over the whole country. Most carry a `+00?` code,
    // but some are numbered (`ABW-5150` Aruba, `PRI-5260` Puerto Rico), and
    // `FRO-1443` even names itself after one Faroese region while covering all
    // of them — so the unit count is the test, not the code.
    let present: BTreeSet<&str> = admin0.iter().map(|f| f.adm0_a3.as_str()).collect();
    let mut units_of: BTreeMap<&str, u32> = BTreeMap::new();
    for f in &kept {
        *units_of.entry(f.adm0_a3.as_str()).or_default() += 1;
    }
    let coextensive: BTreeSet<String> = kept
        .iter()
        .filter(|f| units_of[f.adm0_a3.as_str()] == 1 && present.contains(f.adm0_a3.as_str()))
        .map(|f| f.adm1_code.clone())
        .collect();
    let mut kept: Vec<Admin1Feature> = kept
        .into_iter()
        .filter(|f| !coextensive.contains(&f.adm1_code))
        .collect();

    // Merge sources into their targets. Both ends must still exist
    let wv = worldview.spec().id;
    let mut merged_sources: Vec<MergedSource> = Vec::new();
    for rule in policy.merge.iter().filter(|r| r.worldview == wv) {
        let live: BTreeSet<&str> = kept.iter().map(|f| f.adm1_code.as_str()).collect();
        let dead = stale_listed_codes([rule.code.as_str(), rule.into.as_str()], &live);
        if !dead.is_empty() {
            bail!(
                "geo_policy.toml `merge` names {} for worldview `{wv}`, which does not ship — \
                 drop or fix the row",
                dead.join(", "),
            );
        }
        let src = kept
            .iter()
            .position(|f| f.adm1_code == rule.code)
            .expect("checked live above");
        let src = kept.remove(src);
        let dst = kept
            .iter_mut()
            .find(|f| f.adm1_code == rule.into)
            .expect("checked live above");
        dst.geometry.0.extend(src.geometry.0.iter().cloned());
        merged_sources.push(MergedSource {
            code: src.adm1_code,
            target: rule.into.clone(),
            geometry: src.geometry,
        });
    }

    Ok(Admin1Policy {
        features: kept,
        merged_sources,
    })
}

fn tier(kind: GeoEntityKind) -> u8 {
    match kind {
        GeoEntityKind::Continent => 0,
        GeoEntityKind::Admin0 => 1,
        GeoEntityKind::Admin1 => 2,
        GeoEntityKind::Admin2 => 3,
    }
}

pub fn validate_tier_order(entities: &[GeoEntity]) -> Result<()> {
    let by_id: BTreeMap<u32, &GeoEntity> = entities.iter().map(|e| (e.id.0, e)).collect();
    let bad: Vec<String> = entities
        .iter()
        .filter_map(|e| {
            let parent = by_id.get(&e.parent_id?.0)?;
            (tier(e.kind) <= tier(parent.kind)).then(|| {
                format!(
                    "`{}` ({:?}) under `{}` ({:?})",
                    e.canonical_code, e.kind, parent.canonical_code, parent.kind
                )
            })
        })
        .collect();
    if !bad.is_empty() {
        bail!("tier order violated:\n  {}", bad.join("\n  "));
    }
    Ok(())
}

pub fn stale_listed_codes<'a>(
    listed: impl IntoIterator<Item = &'a str>,
    live: &BTreeSet<&str>,
) -> Vec<String> {
    listed
        .into_iter()
        .filter(|code| !live.contains(code))
        .map(str::to_string)
        .collect()
}

pub fn validate_curated_tables(
    worldview: Worldview,
    province_codes: &BTreeSet<&str>,
) -> Result<()> {
    let policy = policy::get()?;
    let wv = worldview.spec().id;
    let mut problems: Vec<String> = Vec::new();
    for (table, listed) in [
        (
            "geo_policy.toml `unparented`",
            policy
                .unparented
                .iter()
                .filter(|e| e.worldview == wv)
                .map(|e| e.code.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "geo_policy.toml `overrule`",
            policy
                .overrule
                .iter()
                .filter(|e| e.worldview == wv)
                .map(|e| e.code.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        let dead = stale_listed_codes(listed, province_codes);
        if !dead.is_empty() {
            problems.push(format!(
                "{table} lists {} for worldview `{wv}`, which no longer ships — drop the dead \
                 entr{}",
                dead.join(", "),
                if dead.len() == 1 { "y" } else { "ies" },
            ));
        }
    }
    if !problems.is_empty() {
        bail!("curated tables are stale:\n  {}", problems.join("\n  "));
    }
    Ok(())
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
    let mut demoted_features: Vec<&Admin0Feature> = Vec::new();
    for f in features {
        if f.demoted_into.is_some() {
            demoted_features.push(f);
        } else {
            groups.entry(f.adm0_a3.clone()).or_default().push(f);
        }
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
            kind: GeoEntityKind::Admin0,
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

    let mut demoted: BTreeMap<String, DemotedProvince> = BTreeMap::new();
    let mut province_ids: BTreeMap<String, GeoEntityId> = BTreeMap::new();
    let mut province_declared_adm0: BTreeMap<String, String> = BTreeMap::new();
    let mut geometry_for_province: BTreeMap<String, MultiPolygon<f64>> = BTreeMap::new();
    for f in demoted_features {
        let target = f.demoted_into.clone().expect("routed here by the tag");
        if demoted.contains_key(&f.adm0_a3) {
            bail!(
                "demoted `{}` collapses several features — demotion assumes NE ships one",
                f.adm0_a3
            );
        }
        province_ids.insert(f.adm0_a3.clone(), registry.id_for_country(&f.adm0_a3)?);
        province_declared_adm0.insert(f.adm0_a3.clone(), target);
        geometry_for_province.insert(f.adm0_a3.clone(), f.geometry.clone());
        demoted.insert(
            f.adm0_a3.clone(),
            DemotedProvince {
                iso_a3_eh: Some(f.iso_a3_eh.clone()).filter(|code| code != "-99"),
            },
        );
    }

    Ok(EntityModel {
        entities,
        geometry_for_country,
        geometry_for_province,
        province_ids,
        province_declared_adm0,
        demoted,
    })
}

/// Apply the `synthesize` policy rows: for a territory this worldview's
/// admin-0 erased outright (nothing left to demote), build the admin-1 entity
/// a demote would have produced
pub fn apply_synthesized(
    model: &mut EntityModel,
    worldview: Worldview,
    admin1_path: &std::path::Path,
    registry: &Registry,
) -> Result<()> {
    let policy = policy::get()?;
    let wv = worldview.spec().id;
    for rule in policy.synthesize.iter().filter(|r| r.worldview == wv) {
        if model.geometry_for_country.contains_key(&rule.code)
            || model.demoted.contains_key(&rule.code)
        {
            bail!(
                "geo_policy.toml `synthesize` names `{}`, which this worldview's admin-0 still \
                 ships — use an `absorb` demote row instead",
                rule.code
            );
        }
        if !model.geometry_for_country.contains_key(&rule.into) {
            bail!(
                "geo_policy.toml `synthesize` targets `{}`, which is no country in worldview \
                 `{wv}`",
                rule.into
            );
        }
        let members = crate::admin1::parse_admin1_members(admin1_path, &rule.code)?;
        if members.is_empty() {
            bail!(
                "geo_policy.toml `synthesize` row `{}` has no member units in the admin-1 \
                 source — drop the dead row",
                rule.code
            );
        }
        let polys: Vec<geo_types::Polygon<f64>> =
            members.into_iter().flat_map(|m| m.geometry.0).collect();
        model
            .province_ids
            .insert(rule.code.clone(), registry.id_for_country(&rule.code)?);
        model
            .province_declared_adm0
            .insert(rule.code.clone(), rule.into.clone());
        model
            .geometry_for_province
            .insert(rule.code.clone(), MultiPolygon(polys));
        model.demoted.insert(
            rule.code.clone(),
            DemotedProvince {
                iso_a3_eh: rule.iso_a3_eh.clone(),
            },
        );
    }
    Ok(())
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

pub fn resolve_province_parents(
    model: &EntityModel,
    tally: &Coverage,
    worldview: Worldview,
) -> Result<BTreeMap<GeoEntityId, GeoEntityId>> {
    let policy = policy::get()?;
    let wv = worldview.spec().id;
    let mut id_of_country: BTreeMap<&str, GeoEntityId> = BTreeMap::new();
    let mut code_of_country: BTreeMap<GeoEntityId, &str> = BTreeMap::new();
    for entity in &model.entities {
        if matches!(entity.kind, GeoEntityKind::Admin0) {
            id_of_country.insert(entity.canonical_code.as_str(), entity.id);
            code_of_country.insert(entity.id, entity.canonical_code.as_str());
        }
    }
    let overruled: BTreeMap<&str, &str> = policy
        .overrule
        .iter()
        .filter(|e| e.worldview == wv)
        .map(|e| (e.code.as_str(), e.into.as_str()))
        .collect();
    let landless_allowed: BTreeSet<&str> = policy
        .unparented
        .iter()
        .filter(|e| e.worldview == wv)
        .map(|e| e.code.as_str())
        .collect();

    let mut parents: BTreeMap<GeoEntityId, GeoEntityId> = BTreeMap::new();
    let mut dropped: BTreeSet<&str> = BTreeSet::new();
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
             Either this worldview's admin-0 leaves that land unclaimed, in which case add an \
             `unparented` row for `{wv}` to geo_policy.toml with a note, or its admin-0 \
             deliberately places the province in the country named above, which belongs in \
             `overrule`",
            landless.len(),
            landless.join("\n    "),
        ));
    }
    if !stale_overrules.is_empty() {
        problems.push(format!(
            "geo_policy.toml `overrule` has {} entr{} the mask no longer supports:\n    {}\n  \
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
            "geo_policy.toml `unparented` lists {} which the country mask now gives land to — \
             drop the stale entr{} so the province ships",
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
        let (name_key, iso_a3_eh) = match model.demoted.get(adm1_code) {
            // A demoted territory keeps its country-namespace identity.
            Some(d) => (format!("country.{adm1_code}"), d.iso_a3_eh.clone()),
            None => (format!("province.{adm1_code}"), None),
        };
        model.entities.push(GeoEntity {
            id,
            kind: GeoEntityKind::Admin1,
            canonical_code: adm1_code.clone(),
            iso_a3_eh,
            name_key,
            parent_id: Some(parent_id),
            total_area_m2: 0,
        });
    }
    model.entities.sort_by_key(|e| e.id.0);
}
