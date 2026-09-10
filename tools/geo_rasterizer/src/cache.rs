//! Provenance-hash helpers used by the smart-skip cache.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::registry::Namespace;

/// SHA-256 identifying the data instance this run would produce:
/// `GEO_DATA_VERSION` (4 LE bytes, a domain-separation salt), the raw bytes
/// of `geojson_path`, `admin1_path`,
/// every per-namespace file under `registry_dir` (in `Namespace::ALL` order)
/// and `policy_path`, then the asset's `worldview_id` (length-prefixed), in
/// that order.
///
///
/// `worldview_id` is folded in (length-prefixed) so a bin retagged to a
/// different worldview rebuilds even if the geojson/registry are unchanged.
pub fn compute_provenance_hash(
    geojson_path: &Path,
    admin1_path: &Path,
    registry_dir: &Path,
    policy_path: &Path,
    worldview_id: &str,
) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(geo_data_format::GEO_DATA_VERSION.to_le_bytes());
    let registry_files = Namespace::ALL.map(|ns| registry_dir.join(ns.file_name()));
    let paths = [geojson_path, admin1_path]
        .into_iter()
        .chain(registry_files.iter().map(PathBuf::as_path))
        .chain([policy_path]);
    for path in paths {
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
    // Length-prefixed so the id boundary can't collide with the preceding data.
    hasher.update((worldview_id.len() as u32).to_le_bytes());
    hasher.update(worldview_id.as_bytes());
    let out = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&out);
    Ok(hash)
}
