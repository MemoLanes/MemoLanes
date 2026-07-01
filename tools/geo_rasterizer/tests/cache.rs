use std::io::Write;

use geo_data_format::{write_geo_data, TileMembership, MAGIC, TILE_COUNT};
use geo_rasterizer::cache::{compute_provenance_hash, read_existing_hash};

fn write_tmp(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
    f
}

/// A minimal but complete, well-formed `geo_data.bin` carrying `hash`.
fn well_formed_bin(hash: [u8; 32]) -> Vec<u8> {
    let tl = vec![TileMembership::None; TILE_COUNT];
    let bl = std::collections::BTreeMap::new();
    write_geo_data(&[], "iso", &tl, &bl, hash).unwrap()
}

#[test]
fn compute_provenance_hash_is_stable() {
    let geo = write_tmp(b"alpha");
    let reg = write_tmp(b"gamma");
    let h1 = compute_provenance_hash(geo.path(), reg.path(), "iso").unwrap();
    let h2 = compute_provenance_hash(geo.path(), reg.path(), "iso").unwrap();
    assert_eq!(h1, h2, "same inputs must hash the same");
}

#[test]
fn compute_provenance_hash_changes_with_input() {
    let geo = write_tmp(b"alpha");
    let reg = write_tmp(b"gamma");
    let reg2 = write_tmp(b"gamma-2");
    let h1 = compute_provenance_hash(geo.path(), reg.path(), "iso").unwrap();
    let h2 = compute_provenance_hash(geo.path(), reg2.path(), "iso").unwrap();
    assert_ne!(h1, h2);
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
