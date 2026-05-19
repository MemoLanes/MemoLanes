use std::path::Path;

use geo_data_format::PROVENANCE_HASH_END;
use geo_rasterizer::cache::{compute_provenance_hash, read_existing_hash};

#[test]
fn compute_provenance_hash_is_stable() {
    let geojson = Path::new("tests/fixtures/synthetic.geojson");
    let toml = Path::new("tests/fixtures/worldviews.toml");
    let registry = Path::new("tests/fixtures/synthetic_registry.toml");
    let h1 = compute_provenance_hash(geojson, toml, registry).unwrap();
    let h2 = compute_provenance_hash(geojson, toml, registry).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn compute_provenance_hash_changes_with_input() {
    let geojson_a = Path::new("tests/fixtures/synthetic.geojson");
    let toml_a = Path::new("tests/fixtures/worldviews.toml");
    let registry = Path::new("tests/fixtures/synthetic_registry.toml");
    let h_a = compute_provenance_hash(geojson_a, toml_a, registry).unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let raw = std::fs::read_to_string(toml_a).unwrap();
    std::fs::write(tmp.path(), raw + "\n# trivial change\n").unwrap();
    let h_b = compute_provenance_hash(geojson_a, tmp.path(), registry).unwrap();
    assert_ne!(h_a, h_b);
}

#[test]
fn read_existing_hash_returns_none_for_missing_file() {
    assert!(read_existing_hash(Path::new("/nonexistent/path"))
        .unwrap()
        .is_none());
}

#[test]
fn read_existing_hash_returns_none_for_short_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"short").unwrap();
    assert!(read_existing_hash(tmp.path()).unwrap().is_none());
}

#[test]
fn read_existing_hash_returns_bytes_for_well_formed_header() {
    // Sectioned format: MAGIC(4) | provenance_hash(32) | rest...
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut data = b"MGEO".to_vec();
    data.extend_from_slice(&[0xAA; 32]);
    data.extend_from_slice(b"trailing-payload");
    std::fs::write(tmp.path(), &data).unwrap();
    let hash = read_existing_hash(tmp.path()).unwrap().unwrap();
    assert_eq!(hash, [0xAA; 32]);
}

#[test]
fn read_existing_hash_none_for_wrong_magic() {
    // First 4 bytes are NOT "MGEO"; PROVENANCE_HASH_END bytes total.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut data = b"XXXX".to_vec();
    data.extend_from_slice(&[0xBB; 32]);
    assert_eq!(data.len(), PROVENANCE_HASH_END);
    std::fs::write(tmp.path(), &data).unwrap();
    assert!(read_existing_hash(tmp.path()).unwrap().is_none());
}

#[test]
fn read_existing_hash_old_layout_does_not_false_match() {
    // Simulate the OLD (hypothetical) layout: MAGIC(4) + version_bytes(4) + real_hash(32).
    // The current reader reads bytes [PROVENANCE_HASH_OFFSET..PROVENANCE_HASH_END] = [4..36]
    // as the hash, which for this layout would be [version_bytes(4) + real_hash[0..28]].
    // That must NOT equal real_hash, proving the offset bug can't silently reuse a
    // stale asset built with a different header layout.
    let real_hash = [0x5A; 32];
    let version_bytes = [2u8, 0, 0, 0];

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut data = b"MGEO".to_vec(); // MAGIC
    data.extend_from_slice(&version_bytes); // old layout had version here
    data.extend_from_slice(&real_hash); // real hash follows version in old layout
    data.extend_from_slice(b"trailing");
    std::fs::write(tmp.path(), &data).unwrap();

    // The reader MUST return something (magic is valid), but it must NOT be real_hash.
    let result = read_existing_hash(tmp.path()).unwrap();
    assert!(
        result != Some(real_hash),
        "read_existing_hash must not return real_hash from an old-layout file; \
         old-layout file would trigger regeneration (cache miss), not a silent hit. \
         got: {result:?}"
    );
}
