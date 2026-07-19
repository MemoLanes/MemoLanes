//! Build the localized region-name maps consumed by the app via
//! easy_localization: one logical `name_key -> name` map per locale, unioned
//! across worldviews and written as nested JSON (`{"country": {"CHN": …}}`) —
//! the same shape as the UI translation files, and the shape easy_localization
//! resolves natively.
//!
//! The un-prefixed key is worldview-agnostic. Worldviews normally agree on
//! names (Natural Earth's POV files differ in *borders*, not names); when a
//! future source bump makes them diverge, a worldview-scoped override supplies
//! the divergent worldview's name under a `<worldview>.<name_key>` key the app
//! prefers, and every worldview still reading the shared key must agree on its
//! value — generation fails otherwise.
//!
//! Resolution of the shared key, in order:
//!   1. worldview-agnostic override
//!   2. the locale's Natural Earth field on the group's sovereign member —
//!      required to agree across every worldview without a scoped override
//!   3. hard error
//!
//! Scoped overrides additionally emit the `<worldview>.<name_key>` keys.

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
    overrides: &Overrides,
) -> Result<BTreeMap<Locale, BTreeMap<String, String>>> {
    // 1. Collect the entity key set + each country's sovereign localized names,
    //    kept per worldview so divergence can be judged against the overrides.
    let mut continent_codes: BTreeSet<&'static str> = BTreeSet::new();
    let mut country_codes: BTreeSet<String> = BTreeSet::new();
    let mut country_localized: BTreeMap<String, BTreeMap<Worldview, BTreeMap<String, String>>> =
        BTreeMap::new();

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
            if let Some(sov) = sovereign_member(group) {
                country_localized
                    .entry((*adm0).to_string())
                    .or_default()
                    .insert(*worldview, sov.localized_names.clone());
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

    // 3. Flat map per locale, plus worldview-prefixed keys for scoped overrides.
    let mut out: BTreeMap<Locale, BTreeMap<String, String>> = BTreeMap::new();
    for &locale in Locale::ALL {
        let mut names: BTreeMap<String, String> = BTreeMap::new();

        for code in &continent_codes {
            let key = format!("continent.{code}");
            // Continents have no NE feature — the override is the ONLY source.
            let name = overrides.get_default(&key, locale).ok_or_else(|| {
                anyhow!(
                    "no name for `{key}` (locale={}): continents are synthesized and have no \
                     Natural Earth feature, so every continent name must be authored in \
                     geo_names_overrides.toml",
                    locale.spec().tag
                )
            })?;
            names.insert(key, name.to_string());
        }

        for adm0 in &country_codes {
            let key = format!("country.{adm0}");
            let name = resolve_country_name(&key, adm0, locale, &country_localized, overrides)?;
            names.insert(key, name);
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
    Ok(out)
}

fn resolve_country_name(
    key: &str,
    adm0: &str,
    locale: Locale,
    country_localized: &BTreeMap<String, BTreeMap<Worldview, BTreeMap<String, String>>>,
    overrides: &Overrides,
) -> Result<String> {
    if let Some(name) = overrides.get_default(key, locale) {
        return Ok(name.to_string());
    }
    let ne_field = locale.spec().ne_field;
    let by_worldview = country_localized.get(adm0);
    let ne_name = |worldview: Worldview| -> Option<&str> {
        by_worldview?
            .get(&worldview)?
            .get(ne_field)
            .map(String::as_str)
    };

    // NE name -> worldviews carrying it, over the worldviews with no scoped
    // override for this locale (the ones that resolve through the shared key).
    let mut unscoped: BTreeMap<&str, Vec<&'static str>> = BTreeMap::new();
    for &worldview in Worldview::ALL {
        if overrides.get_scoped(key, worldview, locale).is_some() {
            continue;
        }
        if let Some(name) = ne_name(worldview) {
            unscoped.entry(name).or_default().push(worldview.spec().id);
        }
    }
    match unscoped.len() {
        1 => Ok(unscoped.keys().next().unwrap().to_string()),
        0 => Worldview::ALL
            .iter()
            .find_map(|&worldview| ne_name(worldview))
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow!(
                    "no name for `{key}` (locale={}): Natural Earth has no non-empty \
                     `{ne_field}` on this group's sovereign member (a collapsed group with no \
                     `TYPE == \"Country\"` member has none) — add an override to \
                     geo_names_overrides.toml",
                    locale.spec().tag
                )
            }),
        _ => {
            let variants = unscoped
                .iter()
                .map(|(name, worldviews)| format!("`{name}` ({})", worldviews.join(", ")))
                .collect::<Vec<_>>()
                .join(" vs ");
            bail!(
                "Natural Earth names for `{key}` (locale={}) diverge across worldviews sharing \
                 the un-prefixed key: {variants} — author a worldview-scoped override \
                 (`[\"{key}\".<worldview>]`) for the divergent worldviews, or a \
                 worldview-agnostic override",
                locale.spec().tag
            )
        }
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
