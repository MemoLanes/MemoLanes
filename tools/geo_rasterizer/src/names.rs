use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use geo_data_format::{Locale, Worldview};

use crate::admin0::Admin0Feature;
use crate::admin1::Admin1Feature;
use crate::atomic_write::write_atomically;
use crate::entities::{group_continent_code, sovereign_member};
use crate::overrides::Overrides;

pub fn region_names_path(dir: &Path, locale: Locale) -> PathBuf {
    dir.join(format!("region_names.{}.json", locale.spec().tag))
}

pub fn build_region_names(
    by_worldview: &[(Worldview, Vec<Admin0Feature>)],
    admin1_by_worldview: &[(Worldview, Vec<Admin1Feature>)],
    cldr: &BTreeMap<Locale, BTreeMap<String, String>>,
    cldr_subdivisions: &BTreeMap<Locale, BTreeMap<String, String>>,
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
        let mut groups: BTreeMap<&str, Vec<&Admin0Feature>> = BTreeMap::new();
        for f in features {
            groups.entry(f.adm0_a3.as_str()).or_default().push(f);
        }
        for group in groups.values() {
            continent_codes.insert(group_continent_code(group)?);
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
    let province_sources = province_name_sources(admin1_by_worldview);
    // drop ambiguous and let them fallback to NE's names
    let ambiguous = ambiguous_subdivision_keys(admin1_by_worldview);
    let cldr_subdivisions: BTreeMap<Locale, BTreeMap<String, String>> = cldr_subdivisions
        .iter()
        .map(|(locale, names)| {
            let usable = names
                .iter()
                .filter(|(key, _)| !ambiguous.contains(key.as_str()))
                .map(|(key, name)| (key.clone(), name.clone()))
                .collect();
            (*locale, usable)
        })
        .collect();

    // 2. A dead override (typo'd or removed entity) fails the build — it would
    //    otherwise ship silently as an unused key.
    let minted: BTreeSet<String> = continent_codes
        .iter()
        .map(|code| format!("continent.{code}"))
        .chain(country_codes.iter().map(|adm0| format!("country.{adm0}")))
        .chain(
            province_sources
                .keys()
                .map(|adm1_code| format!("province.{adm1_code}")),
        )
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

        for (adm1_code, source) in &province_sources {
            let key = format!("province.{adm1_code}");
            match resolve_province_name(
                &key,
                source,
                locale,
                cldr_subdivisions.get(&locale),
                overrides,
            ) {
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

pub fn subdivision_key(iso_3166_2: &str) -> String {
    iso_3166_2.replace('-', "").to_lowercase()
}

struct ProvinceNameSource {
    subdivision_key: String,
    name_en: Option<String>,
    name_zh: Option<String>,
}

impl ProvinceNameSource {
    fn ne_name(&self, locale: Locale) -> Option<&str> {
        match locale {
            Locale::EnUs => self.name_en.as_deref(),
            Locale::ZhCn => self.name_zh.as_deref(),
        }
    }
}

fn province_name_sources(
    by_worldview: &[(Worldview, Vec<Admin1Feature>)],
) -> BTreeMap<String, ProvinceNameSource> {
    let mut out: BTreeMap<String, ProvinceNameSource> = BTreeMap::new();
    for (_, features) in by_worldview {
        for f in features {
            out.entry(f.adm1_code.clone())
                .or_insert_with(|| ProvinceNameSource {
                    subdivision_key: subdivision_key(&f.iso_3166_2),
                    name_en: f.name_en.clone(),
                    name_zh: f.name_zh.clone(),
                });
        }
    }
    out
}

fn ambiguous_subdivision_keys(
    by_worldview: &[(Worldview, Vec<Admin1Feature>)],
) -> BTreeSet<String> {
    let mut claims: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for (_, features) in by_worldview {
        for f in features {
            claims
                .entry(subdivision_key(&f.iso_3166_2))
                .or_default()
                .insert(f.adm1_code.as_str());
        }
    }
    claims
        .into_iter()
        .filter(|(_, codes)| codes.len() > 1)
        .map(|(key, _)| key)
        .collect()
}

fn resolve_province_name(
    key: &str,
    feature: &ProvinceNameSource,
    locale: Locale,
    cldr_subdivisions: Option<&BTreeMap<String, String>>,
    overrides: &Overrides,
) -> Result<String> {
    if let Some(name) = overrides.get_default(key, locale) {
        return Ok(name.to_string());
    }
    if let Some(name) = cldr_subdivisions.and_then(|m| m.get(&feature.subdivision_key)) {
        return Ok(name.clone());
    }
    if let Some(name) = feature.ne_name(locale) {
        return Ok(name.to_string());
    }
    bail!(
        "no name for `{key}` (locale={}): Natural Earth has no `{}` for this feature, and CLDR \
         has no subdivision `{}` belonging to it alone — add an override to \
         geo_names_overrides.toml",
        locale.spec().tag,
        locale.ne_name_field(),
        feature.subdivision_key,
    )
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
