use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::atomic_write::write_atomically;
use anyhow::{ensure, Context, Result};
use geo_data_format::{DisplayAsset, GeoEntity, GeoEntityKind, Worldview};
use sha2::{Digest, Sha256};

// Bump when exporter behavior changes the generated display bytes. This
// invalidates build metadata, not the display file format or raster provenance.
const DISPLAY_RECIPE_VERSION: u32 = 1;
const VERSION_END: usize = std::mem::size_of::<u32>();
const ASSET_HASH_END: usize = VERSION_END + 32;
const STAMP_LEN: usize = ASSET_HASH_END + 32;

// The asset directory is bundled by Flutter, but its subdirectories are not
// included unless explicitly listed. Each output gets its own build stamp.
fn stamp_path(path: &Path) -> Option<PathBuf> {
    Some(
        path.with_file_name(".build-metadata")
            .join(path.file_name()?),
    )
}

// Domain-separated hash of the geometry selectors and output IDs in the
// exact order consumed by export_records. Provenance covers the source files.
pub(super) struct RasterEntitiesHash(Sha256);

impl Default for RasterEntitiesHash {
    fn default() -> Self {
        Self(Sha256::new_with_prefix(
            b"geo_rasterizer/display-raster-entities/v1\0",
        ))
    }
}

impl RasterEntitiesHash {
    pub(super) fn add_worldview(&mut self, worldview: Worldview, entities: &[GeoEntity]) {
        // Zero marks a worldview boundary, distinct from all source-kind tags.
        self.0.update([0]);
        self.add_str(worldview.spec().id);
        for entity in entities {
            let kind = match entity.kind {
                GeoEntityKind::Continent => continue,
                GeoEntityKind::Admin0 => 1,
                GeoEntityKind::Admin1 => 2,
                GeoEntityKind::Admin2 => 3,
            };
            self.0.update([kind]);
            self.0.update(entity.id.0.to_le_bytes());
            self.add_str(&entity.canonical_code);
        }
    }

    fn add_str(&mut self, value: &str) {
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value.as_bytes());
    }

    pub(super) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

// Resolve symlinked parents even when an output hasn't been written yet.
fn resolved_output(path: &Path) -> Result<PathBuf> {
    match path.canonicalize() {
        Ok(resolved) => Ok(resolved),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let name = path
                .file_name()
                .with_context(|| format!("resolving {}", path.display()))?;
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            Ok(resolved_output(parent.unwrap_or(Path::new(".")))?.join(name))
        }
        Err(err) => Err(err).with_context(|| format!("resolving {}", path.display())),
    }
}

pub(super) fn validate_paths<'a>(
    path: &Path,
    rasters: impl IntoIterator<Item = &'a Path>,
) -> Result<()> {
    let stamp = stamp_path(path).context("display output must have a filename")?;
    let destination = resolved_output(path)?;
    let metadata = resolved_output(&stamp)?;
    for raster in rasters {
        let raster = resolved_output(raster)?;
        ensure!(
            destination != raster,
            "display destination is a raster: {}",
            path.display()
        );
        ensure!(
            metadata != raster,
            "display stamp is a raster: {}",
            stamp.display()
        );
    }
    ensure!(
        destination != metadata,
        "display destination is its own stamp: {}",
        path.display()
    );
    Ok(())
}

// Stamp contents: the LE recipe version, SHA-256 of the asset, and raster entity hash.
pub(super) fn is_current(
    path: &Path,
    manifest: &[(Worldview, [u8; 32])],
    raster_entities_hash: [u8; 32],
) -> bool {
    let Some(stamp_path) = stamp_path(path) else {
        return false;
    };
    let Ok(saved) = fs::read(stamp_path) else {
        return false;
    };
    if saved.len() != STAMP_LEN
        || saved[..VERSION_END] != DISPLAY_RECIPE_VERSION.to_le_bytes()
        || saved[ASSET_HASH_END..] != raster_entities_hash
    {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    if saved[VERSION_END..ASSET_HASH_END] != Sha256::digest(&bytes)[..] {
        return false;
    }
    let Ok(asset) = DisplayAsset::open(path) else {
        return false;
    };
    // Check both directions: extra worldviews are stale too.
    Worldview::ALL.iter().all(|&w| {
        asset.raster_provenance(w)
            == manifest
                .iter()
                .find(|&&(candidate, _)| candidate == w)
                .map(|&(_, hash)| hash)
    })
}

pub(super) fn invalidate(path: &Path) -> Result<()> {
    let stamp_path = stamp_path(path).context("display output must have a filename")?;
    match fs::remove_file(&stamp_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("removing {}", stamp_path.display())),
    }
}

pub(super) fn publish(path: &Path, bytes: &[u8], raster_entities_hash: [u8; 32]) -> Result<()> {
    let stamp_path = stamp_path(path).context("display output must have a filename")?;
    let mut stamp = [0; STAMP_LEN];
    stamp[..VERSION_END].copy_from_slice(&DISPLAY_RECIPE_VERSION.to_le_bytes());
    stamp[VERSION_END..ASSET_HASH_END].copy_from_slice(&Sha256::digest(bytes));
    stamp[ASSET_HASH_END..].copy_from_slice(&raster_entities_hash);
    write_atomically(path, bytes)?;
    write_atomically(&stamp_path, &stamp)
}
