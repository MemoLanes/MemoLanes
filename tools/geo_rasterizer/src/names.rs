//! Build the localized region-name maps consumed by the app via
//! easy_localization: one flat `name_key -> name` map per locale, unioned across
//! worldviews.
//!
//! Names are worldview-INVARIANT (Natural Earth's POV files agree on names; only
//! borders differ), so the map is keyed by `name_key` alone. The only source of a
//! per-worldview NAME is a hand-authored worldview-scoped override, emitted under
//! a `<worldview>.<name_key>` key — none today; the mechanism is latent for
//! future disputed admin-1 provinces.
//!
//! Resolution, in order:
//!   1. worldview-scoped override → the `<worldview>.<name_key>` prefixed key
//!   2. worldview-agnostic override → the flat key
//!   3. the locale's Natural Earth field on the group's sovereign member
//!   4. hard error

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
    // 1. Collect the entity key set + each country's sovereign localized names.
    let mut continent_codes: BTreeSet<&'static str> = BTreeSet::new();
    let mut country_codes: BTreeSet<String> = BTreeSet::new();
    let mut country_localized: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    for (_wv, features) in by_worldview {
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
                use std::collections::btree_map::Entry;
                match country_localized.entry((*adm0).to_string()) {
                    Entry::Vacant(slot) => {
                        slot.insert(sov.localized_names.clone());
                    }
                    Entry::Occupied(slot) if *slot.get() != sov.localized_names => bail!(
                        "Natural Earth names for `{adm0}` diverge across worldviews — the flat \
                         region-name design assumes worldview-invariant names. Author a \
                         per-worldview override (`[\"country.{adm0}.name\".<worldview>]`), which \
                         emits a `<worldview>.country.{adm0}.name` key the app prefers, or \
                         reconsider the flat design."
                    ),
                    Entry::Occupied(_) => {}
                }
            }
        }
    }

    // 2. Flat map per locale, plus worldview-prefixed keys for scoped overrides.
    let mut out: BTreeMap<Locale, BTreeMap<String, String>> = BTreeMap::new();
    for &locale in Locale::ALL {
        let mut names: BTreeMap<String, String> = BTreeMap::new();

        for code in &continent_codes {
            let key = format!("continent.{code}.name");
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
            let key = format!("country.{adm0}.name");
            let ne_field = locale.spec().ne_field;
            let name = overrides
                .get_default(&key, locale)
                .or_else(|| {
                    country_localized
                        .get(adm0)
                        .and_then(|ln| ln.get(ne_field))
                        .map(String::as_str)
                })
                .ok_or_else(|| {
                    anyhow!(
                        "no name for `{key}` (locale={}): Natural Earth has no non-empty \
                         `{ne_field}` on this group's sovereign member (a collapsed group with no \
                         `TYPE == \"Country\"` member has none) — add an override to \
                         geo_names_overrides.toml",
                        locale.spec().tag
                    )
                })?;
            names.insert(key, name.to_string());
        }

        // Worldview-scoped overrides → `<worldview>.<name_key>`, only where a
        // scoped value genuinely exists for THIS locale (else the flat key wins).
        for (key, worldview) in overrides.scoped_keys() {
            if let Some(name) = overrides.get_scoped(key, worldview, locale) {
                names.insert(format!("{}.{key}", worldview.spec().id), name.to_string());
            }
        }

        out.insert(locale, names);
    }
    Ok(out)
}

pub fn write_region_names(
    dir: &Path,
    locale: Locale,
    names: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    let path = region_names_path(dir, locale);
    let mut bytes = serde_json::to_vec_pretty(names).context("serializing region names")?;
    bytes.push(b'\n');
    write_atomically(&path, &bytes)?;
    Ok(path)
}
