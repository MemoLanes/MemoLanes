//! Build the localized region-name maps consumed by the app via
//! easy_localization: one logical `name_key -> name` map per locale, unioned
//! across worldviews and written as nested JSON (`{"country": {"CHN": …}}`) —
//! the same shape as the UI translation files, and the shape easy_localization
//! resolves natively.
//!
//! Names come from Unicode CLDR (`territories.json`, keyed by ISO 3166-1
//! alpha-2), joined to each entity via the sovereign feature's `ISO_A2_EH`.
//! CLDR names are worldview-independent, so a country resolves to one name
//! across every worldview by construction — the un-prefixed key is
//! worldview-agnostic. (A given `ADM0_A3` must therefore carry the same
//! `ISO_A2_EH` in every worldview; generation fails otherwise.)
//!
//! Resolution of the shared key, in order:
//!   1. worldview-agnostic override
//!   2. the CLDR name for the group's sovereign `ISO_A2_EH`
//!   3. hard error — no silent fallback
//!
//! Overrides exist where CLDR has no usable entry (continents have no
//! territory; a collapsed group has no sovereign `ISO_A2_EH`; NE-only
//! aggregates like the Spratlys) or where CLDR's name is not the one we ship. A
//! worldview-scoped override additionally emits a `<worldview>.<name_key>` key
//! the app prefers — implemented for future admin-1 use, where a disputed
//! region legitimately differs by political view.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use geo_data_format::{Locale, Worldview};

use crate::atomic_write::write_atomically;
use crate::entities::continent_code_pub;
use crate::overrides::Overrides;
use crate::parse::ParsedFeature;

fn sovereign_member<'a>(group: &[&'a ParsedFeature]) -> Option<&'a ParsedFeature> {
    match group {
        [only] => Some(only),
        members => members
            .iter()
            .copied()
            .find(|f| f.feature_type == "Country"),
    }
}

pub fn region_names_path(dir: &Path, locale: Locale) -> PathBuf {
    dir.join(format!("region_names.{}.json", locale.spec().tag))
}

pub fn build_region_names(
    by_worldview: &[(Worldview, Vec<ParsedFeature>)],
    cldr: &BTreeMap<Locale, BTreeMap<String, String>>,
    overrides: &Overrides,
) -> Result<BTreeMap<Locale, BTreeMap<String, String>>> {
    // 1. Collect the entity key set + each country's sovereign `ISO_A2_EH` (the
    //    CLDR join key). CLDR names don't depend on worldview, so we keep one
    //    alpha-2 per ADM0_A3 and require every worldview carrying a sovereign
    //    member to agree on it — a code must denote one territory.
    let mut continent_codes: BTreeSet<&'static str> = BTreeSet::new();
    let mut country_codes: BTreeSet<String> = BTreeSet::new();
    let mut country_a2: BTreeMap<String, (String, &'static str)> = BTreeMap::new();

    for (worldview, features) in by_worldview {
        let mut groups: BTreeMap<&str, Vec<&ParsedFeature>> = BTreeMap::new();
        for f in features {
            groups.entry(f.adm0_a3.as_str()).or_default().push(f);
        }
        for group in groups.values() {
            continent_codes.insert(continent_code_pub(&group[0].continent, &group[0].region_un));
        }
        for (adm0, group) in &groups {
            country_codes.insert((*adm0).to_string());
            let a2 = sovereign_member(group)
                .map(|sov| sov.iso_a2_eh.as_str())
                .filter(|a2| *a2 != "-99");
            if let Some(a2) = a2 {
                match country_a2.get(*adm0) {
                    None => {
                        country_a2
                            .insert((*adm0).to_string(), (a2.to_string(), worldview.spec().id));
                    }
                    Some((seen, seen_wv)) if seen != a2 => bail!(
                        "ADM0_A3 `{adm0}` maps to different ISO_A2_EH across worldviews: \
                         `{seen}` ({seen_wv}) vs `{a2}` ({}) — a code must denote one territory",
                        worldview.spec().id
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    // 2. A dead override (typo'd or removed entity) fails the build — it would
    //    otherwise ship silently as an unused key.
    let minted: BTreeSet<String> = continent_codes
        .iter()
        .map(|code| format!("continent.{code}"))
        .chain(country_codes.iter().map(|adm0| format!("country.{adm0}")))
        .collect();
    let dead: Vec<&str> = overrides.keys().filter(|k| !minted.contains(*k)).collect();
    if !dead.is_empty() {
        bail!(
            "geo_names_overrides.toml names entities that exist in no worldview: {} — fix the \
             typo or remove the dead override",
            dead.join(", ")
        );
    }

    let mut cldr_by_a2: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for adm0 in &country_codes {
        let key = format!("country.{adm0}");
        let fully_overridden = Locale::ALL
            .iter()
            .all(|&l| overrides.get_default(&key, l).is_some());
        if fully_overridden {
            continue;
        }
        if let Some((a2, _)) = country_a2.get(adm0) {
            cldr_by_a2
                .entry(a2.as_str())
                .or_default()
                .push(adm0.as_str());
        }
    }
    let collisions: Vec<String> = cldr_by_a2
        .iter()
        .filter(|(_, adms)| adms.len() > 1)
        .map(|(a2, adms)| {
            format!(
                "ISO_A2_EH `{a2}` is shared by {} entities resolving via CLDR ({}) — CLDR names \
                 each after territory `{a2}`; override all but the canonical one in \
                 geo_names_overrides.toml",
                adms.len(),
                adms.join(", ")
            )
        })
        .collect();
    if !collisions.is_empty() {
        bail!(
            "region names have {} alpha-2 collision(s):\n  {}",
            collisions.len(),
            collisions.join("\n  ")
        );
    }

    // 3. Flat map per locale, plus worldview-prefixed keys for scoped overrides.
    //    Every unfillable key is collected so a single run reports all gaps to
    //    author, rather than one failure per rerun.
    let mut out: BTreeMap<Locale, BTreeMap<String, String>> = BTreeMap::new();
    let mut missing: Vec<String> = Vec::new();
    for &locale in Locale::ALL {
        let cldr_names = cldr.get(&locale).ok_or_else(|| {
            anyhow!(
                "no CLDR territories loaded for locale {}",
                locale.spec().tag
            )
        })?;
        let mut names: BTreeMap<String, String> = BTreeMap::new();

        for code in &continent_codes {
            let key = format!("continent.{code}");
            match overrides.get_default(&key, locale) {
                Some(name) => {
                    names.insert(key, name.to_string());
                }
                None => missing.push(format!(
                    "`{key}` (locale={}): continents are synthesized and have no CLDR territory — \
                     author the name in geo_names_overrides.toml",
                    locale.spec().tag
                )),
            }
        }

        for adm0 in &country_codes {
            let key = format!("country.{adm0}");
            match resolve_country_name(&key, adm0, locale, &country_a2, cldr_names, overrides) {
                Ok(name) => {
                    names.insert(key, name);
                }
                Err(e) => missing.push(e.to_string()),
            }
        }

        // Worldview-scoped overrides → `<worldview>.<name_key>`, only where a
        // scoped value genuinely exists for THIS locale (else the shared key wins).
        for (key, worldview) in overrides.scoped_keys() {
            if let Some(name) = overrides.get_scoped(key, worldview, locale) {
                names.insert(format!("{}.{key}", worldview.spec().id), name.to_string());
            }
        }

        out.insert(locale, names);
    }
    if !missing.is_empty() {
        bail!(
            "region names have {} unresolved gap(s):\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
    }
    Ok(out)
}

fn resolve_country_name(
    key: &str,
    adm0: &str,
    locale: Locale,
    country_a2: &BTreeMap<String, (String, &'static str)>,
    cldr_names: &BTreeMap<String, String>,
    overrides: &Overrides,
) -> Result<String> {
    if let Some(name) = overrides.get_default(key, locale) {
        return Ok(name.to_string());
    }
    match country_a2.get(adm0) {
        None => bail!(
            "no name for `{key}` (locale={}): this group has no usable `ISO_A2_EH` (a collapsed \
             group with no `TYPE == \"Country\"` member, or a `-99` sentinel), so it has no CLDR \
             territory — add an override to geo_names_overrides.toml",
            locale.spec().tag
        ),
        Some((a2, _)) => cldr_names.get(a2).cloned().ok_or_else(|| {
            anyhow!(
                "no name for `{key}` (locale={}): CLDR has no territory `{a2}` (ISO_A2_EH of this \
                 group's sovereign member) — add an override to geo_names_overrides.toml",
                locale.spec().tag
            )
        }),
    }
}

fn nest(flat: &BTreeMap<String, String>) -> Result<serde_json::Value> {
    let mut root = serde_json::Map::new();
    for (key, value) in flat {
        let segments: Vec<&str> = key.split('.').collect();
        let (leaf, parents) = segments.split_last().expect("keys are non-empty");
        let mut node = &mut root;
        for seg in parents {
            node = node
                .entry(seg.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                .as_object_mut()
                .ok_or_else(|| anyhow!("name key `{key}`: segment `{seg}` is already a name"))?;
        }
        // The flat map's keys are unique, so a prior entry can only be a
        // subtree of a longer key this one would truncate.
        if node
            .insert(
                (*leaf).to_string(),
                serde_json::Value::String(value.clone()),
            )
            .is_some()
        {
            bail!("name key `{key}` collides with longer keys nested under it");
        }
    }
    Ok(serde_json::Value::Object(root))
}

pub fn write_region_names(
    dir: &Path,
    locale: Locale,
    names: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    let path = region_names_path(dir, locale);
    let mut bytes = serde_json::to_vec_pretty(&nest(names)?).context("serializing region names")?;
    bytes.push(b'\n');
    write_atomically(&path, &bytes)?;
    Ok(path)
}
