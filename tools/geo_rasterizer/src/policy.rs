use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AbsorbMode {
    Demote,
    Merge,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbsorbRule {
    pub worldview: String,
    pub code: String,
    pub into: String,
    pub mode: AbsorbMode,
    #[serde(default)]
    pub attribute_to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesizeRule {
    pub worldview: String,
    pub code: String,
    pub into: String,
    /// Authored: the member admin-1 units carry no `ISO_A3_EH` of their own.
    #[serde(default)]
    pub iso_a3_eh: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeRule {
    pub worldview: String,
    pub code: String,
    pub into: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldviewCode {
    pub worldview: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverruleRule {
    pub worldview: String,
    pub code: String,
    pub into: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[allow(dead_code)]
    schema: u32,
    pub absorb: Vec<AbsorbRule>,
    #[serde(default)]
    pub synthesize: Vec<SynthesizeRule>,
    pub merge: Vec<MergeRule>,
    pub drop_admin1_in: BTreeSet<String>,
    pub unparented: Vec<WorldviewCode>,
    pub overrule: Vec<OverruleRule>,
}

pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("geo_policy.toml")
}

pub fn load(path: &Path) -> Result<Policy> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let policy: Policy =
        toml::from_str(&raw).map_err(|e| anyhow!("parsing {}: {e}", path.display()))?;
    for rule in &policy.absorb {
        if rule.mode == AbsorbMode::Demote && rule.attribute_to.is_some() {
            bail!(
                "{}: absorb `{}` is a demote with `attribute_to` — the demoted entity carries \
                 its own land; attribution is a merge-only field",
                path.display(),
                rule.code
            );
        }
    }
    Ok(policy)
}

pub fn get() -> Result<&'static Policy> {
    static POLICY: OnceLock<Policy> = OnceLock::new();
    if let Some(policy) = POLICY.get() {
        return Ok(policy);
    }
    let loaded = load(&default_path())?;
    Ok(POLICY.get_or_init(|| loaded))
}
