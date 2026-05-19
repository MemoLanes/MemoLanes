//! Provenance-hash helpers used by the smart-skip cache.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// SHA-256 identifying the data instance this run would produce:
/// `GEO_DATA_VERSION` (4 LE bytes, a domain-separation salt) followed by
/// the raw bytes of `geojson_path`, `worldviews_path`, then
/// `registry_path`, in that order with no separator.
///
/// The version salt is what closes the "same inputs, changed
/// rasterizer/format" hole: bumping `geo_data_format::GEO_DATA_VERSION`
/// changes this hash even when the source files are byte-identical, so
/// the smart-skip rebuilds and any runtime consumer cache
/// invalidates. Inputs-only equivalent: `(printf version; cat geojson
/// worldviews registry) | sha256sum`.
pub fn compute_provenance_hash(
    geojson_path: &Path,
    worldviews_path: &Path,
    registry_path: &Path,
) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(geo_data_format::GEO_DATA_VERSION.to_le_bytes());
    for path in [geojson_path, worldviews_path, registry_path] {
        let f = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut reader = BufReader::new(f);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    let out = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&out);
    Ok(hash)
}

/// Read the embedded provenance hash from an existing `geo_data.bin`.
/// Returns `Ok(None)` if the file is missing or shorter than the
/// header (i.e., not a valid bin we want to compare against).
///
/// Sectioned format layout: `MAGIC(4) | provenance_hash(32) | ...`
/// The hash is therefore at byte offsets
/// `geo_data_format::PROVENANCE_HASH_OFFSET..geo_data_format::PROVENANCE_HASH_END`.
pub fn read_existing_hash(bin_path: &Path) -> Result<Option<[u8; 32]>> {
    let f = match File::open(bin_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context(format!("opening {}", bin_path.display())),
    };
    let mut buf = [0u8; geo_data_format::PROVENANCE_HASH_END];
    let mut reader = BufReader::new(f);
    let mut total = 0;
    while total < geo_data_format::PROVENANCE_HASH_END {
        let n = reader.read(&mut buf[total..])?;
        if n == 0 {
            return Ok(None);
        }
        total += n;
    }
    if &buf[0..geo_data_format::PROVENANCE_HASH_OFFSET] != geo_data_format::MAGIC {
        return Ok(None);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(
        &buf[geo_data_format::PROVENANCE_HASH_OFFSET..geo_data_format::PROVENANCE_HASH_END],
    );
    Ok(Some(hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(content: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content).unwrap();
        f
    }

    #[test]
    fn registry_change_changes_provenance_hash() {
        let geo = tmp(b"GEO");
        let wv = tmp(b"WV");
        let r1 = tmp(b"REG-A");
        let r2 = tmp(b"REG-B");
        let h1 = compute_provenance_hash(geo.path(), wv.path(), r1.path()).unwrap();
        let h2 = compute_provenance_hash(geo.path(), wv.path(), r2.path()).unwrap();
        assert_ne!(h1, h2, "registry must participate in the cache key");
    }
}
