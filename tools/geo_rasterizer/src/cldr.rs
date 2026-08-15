//! Load a pinned CLDR `territories.json` into an alpha-2 → display-name map.
//!
//! Shape (see `geo_data_format::cldr` for the pin):
//! ```json
//! { "main": { "en": { "localeDisplayNames": { "territories": {
//!     "US": "United States", "US-alt-short": "US", "US-alt-…": "…" } } } } }
//! ```
//! Keys are ISO 3166-1 alpha-2 (plus numeric M49 region codes, which never match
//! an `ISO_A2_EH` lookup and so pass through harmlessly). We take the **primary**
//! form only — `<code>-alt-*` variants (short/variant/…) are skipped.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

pub fn load_territories(path: &Path, cldr_tag: &str) -> Result<BTreeMap<String, String>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading CLDR territories at {}", path.display()))?;
    let root: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing CLDR territories at {}", path.display()))?;

    let territories = root
        .get("main")
        .and_then(|m| m.get(cldr_tag))
        .and_then(|l| l.get("localeDisplayNames"))
        .and_then(|d| d.get("territories"))
        .and_then(|t| t.as_object())
        .ok_or_else(|| {
            anyhow!(
                "{}: missing main.{cldr_tag}.localeDisplayNames.territories",
                path.display()
            )
        })?;

    let mut out = BTreeMap::new();
    for (code, name) in territories {
        if code.contains("-alt-") {
            continue;
        }
        let name = name
            .as_str()
            .ok_or_else(|| anyhow!("{}: territory `{code}` is not a string", path.display()))?;
        out.insert(code.clone(), name.to_string());
    }
    if out.is_empty() {
        bail!(
            "{}: main.{cldr_tag}.localeDisplayNames.territories has no primary entries",
            path.display()
        );
    }
    Ok(out)
}

/// Load a pinned CLDR `<tag>.json` from `cldr-subdivisions-full` into a
/// BCP-47 subdivision-key → display-name map.
///
/// Shape (note: no locale level, unlike `territories.json`):
/// ```json
/// { "subdivisions": { "identity": { "language": "en" },
///   "localeDisplayNames": { "subdivisions": { "usca": "California" } } } }
/// ```
pub fn load_subdivisions(path: &Path, cldr_tag: &str) -> Result<BTreeMap<String, String>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading CLDR subdivisions at {}", path.display()))?;
    let root: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing CLDR subdivisions at {}", path.display()))?;
    let node = root
        .get("subdivisions")
        .ok_or_else(|| anyhow!("{}: missing `subdivisions`", path.display()))?;

    let language = node
        .get("identity")
        .and_then(|i| i.get("language"))
        .and_then(|l| l.as_str());
    if language != Some(cldr_tag) {
        bail!(
            "{}: declares language {language:?} but was fetched for `{cldr_tag}`",
            path.display()
        );
    }

    let subdivisions = node
        .get("localeDisplayNames")
        .and_then(|d| d.get("subdivisions"))
        .and_then(|s| s.as_object())
        .ok_or_else(|| {
            anyhow!(
                "{}: missing subdivisions.localeDisplayNames.subdivisions",
                path.display()
            )
        })?;

    let mut out = BTreeMap::new();
    for (code, name) in subdivisions {
        let name = name
            .as_str()
            .ok_or_else(|| anyhow!("{}: subdivision `{code}` is not a string", path.display()))?;
        out.insert(code.clone(), name.to_string());
    }
    if out.is_empty() {
        bail!("{}: no subdivision entries", path.display());
    }
    Ok(out)
}
