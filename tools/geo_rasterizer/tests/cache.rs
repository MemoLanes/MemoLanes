use std::io::Write;
use std::path::{Path, PathBuf};

use geo_data_format::{write_geo_data, TileMembership, MAGIC, TILE_COUNT};
use geo_rasterizer::cache::{compute_provenance_hash, read_existing_hash};

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

/// A minimal but complete, well-formed `geo_data.bin` carrying `hash`.
fn well_formed_bin(hash: [u8; 32]) -> Vec<u8> {
    let tl = vec![TileMembership::None; TILE_COUNT];
    let bl = std::collections::BTreeMap::new();
    write_geo_data(&[], "iso", &tl, &bl, hash).unwrap()
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
fn read_existing_hash_returns_none_for_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does_not_exist.bin");
    assert_eq!(read_existing_hash(&missing).unwrap(), None);
}

#[test]
fn read_existing_hash_returns_none_for_short_file() {
    let f = write_tmp(b"too short");
    assert_eq!(read_existing_hash(f.path()).unwrap(), None);
}

#[test]
fn read_existing_hash_returns_bytes_for_complete_file() {
    let hash = [0xABu8; 32];
    let f = write_tmp(&well_formed_bin(hash));
    assert_eq!(read_existing_hash(f.path()).unwrap(), Some(hash));
}

#[test]
fn read_existing_hash_rejects_torn_file() {
    // A complete header (valid magic + hash) over a truncated body must be
    // rejected: the file size won't match the length the header encodes, so
    // the smart-skip rebuilds instead of trusting the stale hash.
    let bytes = well_formed_bin([0xCDu8; 32]);
    let f = write_tmp(&bytes[..bytes.len() - 1]);
    assert_eq!(read_existing_hash(f.path()).unwrap(), None);
}

#[test]
fn read_existing_hash_old_layout_does_not_false_match() {
    // A file that is exactly MAGIC with no hash must not be accepted.
    let f = write_tmp(MAGIC);
    assert_eq!(read_existing_hash(f.path()).unwrap(), None);
}

#[test]
fn the_committed_policy_file_parses_and_validates() {
    let policy = geo_rasterizer::policy::get().unwrap();
    assert!(!policy.absorb.is_empty());
    assert!(!policy.drop_admin1_in.is_empty());
}
