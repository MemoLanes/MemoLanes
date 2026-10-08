use std::io::Write;
use std::path::{Path, PathBuf};

use geo_rasterizer::cache::compute_provenance_hash;

fn write_tmp(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
    f
}

/// A registry directory whose province file carries `marker`.
fn registry_dir(marker: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("continents.toml"), "schema = 1\n").unwrap();
    std::fs::write(dir.path().join("countries.toml"), "schema = 1\n").unwrap();
    std::fs::write(
        dir.path().join("provinces.toml"),
        format!("schema = 1\n# {marker}\n"),
    )
    .unwrap();
    dir
}

/// All file inputs with distinct content, ready to vary one at a time.
struct Inputs {
    geo: tempfile::NamedTempFile,
    admin1: tempfile::NamedTempFile,
    reg: tempfile::TempDir,
    policy: tempfile::NamedTempFile,
}

fn inputs() -> Inputs {
    Inputs {
        geo: write_tmp(b"alpha"),
        admin1: write_tmp(b"beta"),
        reg: registry_dir("gamma"),
        policy: write_tmp(b"policy"),
    }
}

fn hash(i: &Inputs, worldview: &str) -> [u8; 32] {
    compute_provenance_hash(
        i.geo.path(),
        i.admin1.path(),
        i.reg.path(),
        i.policy.path(),
        worldview,
    )
    .unwrap()
}

fn hash_at(i: &Inputs, admin1: &Path, reg: &Path, policy: &Path, worldview: &str) -> [u8; 32] {
    compute_provenance_hash(i.geo.path(), admin1, reg, policy, worldview).unwrap()
}

#[test]
fn compute_provenance_hash_is_stable() {
    let i = inputs();
    assert_eq!(
        hash(&i, "iso"),
        hash(&i, "iso"),
        "same inputs must hash the same"
    );
}

#[test]
fn compute_provenance_hash_changes_with_each_input() {
    let i = inputs();
    let base = hash(&i, "iso");
    let admin1_b = write_tmp(b"beta-2");
    let reg_b = registry_dir("gamma-2");
    let policy_b = write_tmp(b"policy-2");
    let p = |f: &tempfile::NamedTempFile| PathBuf::from(f.path());
    for (name, changed) in [
        (
            "admin-1 source",
            hash_at(&i, &p(&admin1_b), i.reg.path(), i.policy.path(), "iso"),
        ),
        (
            "registry file",
            hash_at(&i, i.admin1.path(), reg_b.path(), i.policy.path(), "iso"),
        ),
        (
            "geo_policy.toml",
            hash_at(&i, i.admin1.path(), i.reg.path(), &p(&policy_b), "iso"),
        ),
    ] {
        assert_ne!(base, changed, "changing {name} must invalidate the cache");
    }
}

#[test]
fn compute_provenance_hash_changes_with_worldview() {
    let i = inputs();
    assert_ne!(
        hash(&i, "iso"),
        hash(&i, "chn"),
        "worldview id must participate in the cache key"
    );
}

#[test]
fn the_committed_policy_file_parses_and_validates() {
    let policy = geo_rasterizer::policy::get().unwrap();
    assert!(!policy.absorb.is_empty());
    assert!(!policy.drop_admin1_in.is_empty());
}
