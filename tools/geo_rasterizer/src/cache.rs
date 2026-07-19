//! Provenance-hash helpers used by the smart-skip cache.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// SHA-256 identifying the data instance this run would produce:
/// `GEO_DATA_VERSION` (4 LE bytes, a domain-separation salt), the raw bytes
/// of `geojson_path` then `registry_path`, then the asset's `worldview_id`
/// (length-prefixed), in that order.
///
/// The version salt is what closes the "same inputs, changed
/// rasterizer/format" hole: bumping `geo_data_format::GEO_DATA_VERSION`
/// changes this hash even when the inputs are byte-identical, so the
/// smart-skip rebuilds and any runtime consumer cache invalidates.
///
/// `worldview_id` is folded in (length-prefixed) so a bin retagged to a
/// different worldview rebuilds even if the geojson/registry are unchanged.
pub fn compute_provenance_hash(
    geojson_path: &Path,
    registry_path: &Path,
    worldview_id: &str,
) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(geo_data_format::GEO_DATA_VERSION.to_le_bytes());
    for path in [geojson_path, registry_path] {
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

/// Read the embedded provenance hash from an existing `geo_data.bin`.
/// Returns `Ok(None)` if the file is missing, shorter than the header, has
/// the wrong magic, or is torn (its size doesn't match the length its own
/// header implies) — in every case the caller treats it as "no usable
/// cache" and rebuilds.
///
/// The torn-size check is the self-heal: the provenance hash lives at the
/// front of the header (`PROVENANCE_HASH_OFFSET..PROVENANCE_HASH_END`), so a
/// write interrupted after the header but before the body would otherwise
/// present a matching hash on top of a truncated file and poison the
/// smart-skip into never rebuilding. The header fully determines the
/// correct file size (see [`geo_data_format::expected_total_len`]), so we
/// reject any mismatch.
///
/// Sectioned format layout: `MAGIC(4) | provenance_hash(32) | sections | ...`
pub fn read_existing_hash(bin_path: &Path) -> Result<Option<[u8; 32]>> {
    let f = match File::open(bin_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context(format!("opening {}", bin_path.display())),
    };
    let file_len = f
        .metadata()
        .with_context(|| format!("stat {}", bin_path.display()))?
        .len();
    let mut buf = [0u8; geo_data_format::HEADER_LEN];
    let mut reader = BufReader::new(f);
    let mut total = 0;
    while total < geo_data_format::HEADER_LEN {
        let n = reader.read(&mut buf[total..])?;
        if n == 0 {
            return Ok(None);
        }
        total += n;
    }
    // Reject torn writes: a valid header must match the file size it encodes.
    // `expected_total_len` also rejects a bad magic.
    match geo_data_format::expected_total_len(&buf) {
        Some(expected) if expected as u64 == file_len => {}
        _ => return Ok(None),
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(
        &buf[geo_data_format::PROVENANCE_HASH_OFFSET..geo_data_format::PROVENANCE_HASH_END],
    );
    Ok(Some(hash))
}
