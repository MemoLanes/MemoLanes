//! Hand-authored name overrides — the only part of the name pipeline a human
//! writes, and the only part reviewed in a PR.
//!
//! Two reasons an override exists:
//!   * Natural Earth has NO name for the entity. Continents have no feature of
//!     their own, and a collapsed group with no sovereign member (`IOA` in the
//!     iso worldview) has no name in any language.
//!   * Natural Earth's name is not one we ship (e.g. `NAME_ZH` for `TWN`).
//!
//! Shape: a locale entry is a string value, a worldview entry is a sub-table.
//! ```toml
//! ["country.TWN"]        # default: every worldview
//! zh-CN = "台湾"
//!
//! ["country.TWN".usa]    # usa worldview only; beats the default
//! en-US = "Taiwan"
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use geo_data_format::{Locale, Worldview};
use serde::Deserialize;

/// Raw TOML shape: `name_key -> (locale tag | worldview id) -> ...`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawField {
    Name(String),
    PerWorldview(BTreeMap<String, String>),
}

#[derive(Debug, Default)]
struct Entry {
    default: BTreeMap<String, String>,
    per_worldview: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Default)]
pub struct Overrides(BTreeMap<String, Entry>);

impl Overrides {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading overrides at {}", path.display()))?;
        Self::from_toml_str(&raw)
            .with_context(|| format!("parsing overrides at {}", path.display()))
    }

    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let disk: BTreeMap<String, BTreeMap<String, RawField>> =
            toml::from_str(raw).context("parsing overrides TOML")?;

        let mut out: BTreeMap<String, Entry> = BTreeMap::new();
        for (name_key, fields) in disk {
            let mut entry = Entry::default();
            for (field_key, value) in fields {
                match value {
                    RawField::Name(name) => {
                        Locale::from_tag(&field_key).with_context(|| {
                            format!(
                                "in override `{name_key}`: `{field_key}` is not a supported \
                                 locale tag (a string value must be keyed by one; a worldview \
                                 override must be a sub-table)"
                            )
                        })?;
                        check_name(&name_key, &field_key, &name)?;
                        entry.default.insert(field_key, name);
                    }
                    RawField::PerWorldview(by_locale) => {
                        Worldview::from_id(&field_key).with_context(|| {
                            format!(
                                "in override `{name_key}`: `{field_key}` is not a known \
                                 worldview id (a sub-table must be keyed by one)"
                            )
                        })?;
                        for (tag, name) in &by_locale {
                            Locale::from_tag(tag).with_context(|| {
                                format!(
                                    "in override `{name_key}` (worldview `{field_key}`): \
                                     `{tag}` is not a supported locale tag"
                                )
                            })?;
                            check_name(&name_key, tag, name)?;
                        }
                        entry.per_worldview.insert(field_key, by_locale);
                    }
                }
            }
            out.insert(name_key, entry);
        }
        Ok(Overrides(out))
    }

    pub fn get(&self, name_key: &str, worldview: Worldview, locale: Locale) -> Option<&str> {
        let entry = self.0.get(name_key)?;
        let tag = locale.spec().tag;
        entry
            .per_worldview
            .get(worldview.spec().id)
            .and_then(|by_locale| by_locale.get(tag))
            .or_else(|| entry.default.get(tag))
            .map(String::as_str)
    }

    pub fn get_default(&self, name_key: &str, locale: Locale) -> Option<&str> {
        self.0
            .get(name_key)?
            .default
            .get(locale.spec().tag)
            .map(String::as_str)
    }

    pub fn get_scoped(&self, name_key: &str, worldview: Worldview, locale: Locale) -> Option<&str> {
        self.0
            .get(name_key)?
            .per_worldview
            .get(worldview.spec().id)?
            .get(locale.spec().tag)
            .map(String::as_str)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn scoped_keys(&self) -> impl Iterator<Item = (&str, Worldview)> {
        self.0.iter().flat_map(|(name_key, entry)| {
            entry.per_worldview.keys().map(move |id| {
                let wv = Worldview::from_id(id).expect("validated at load");
                (name_key.as_str(), wv)
            })
        })
    }
}

fn check_name(name_key: &str, tag: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("override `{name_key}` / `{tag}` is empty — remove it or give it a name");
    }
    Ok(())
}
