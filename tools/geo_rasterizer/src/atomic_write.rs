//! Atomic publish: stream bytes into a deterministic `<path>.tmp`, fsync,
//! then rename over `path`. The rename is the only thing readers and the
//! smart-skip cache rely on — an observer ever sees the old complete file or
//! the new complete file, never a half-written one.
//!
//! The temp name is intentionally `<path>.tmp` (a fixed sibling), not a
//! random tempfile name:
//!   * `.gitignore` matches `*.bin.tmp` / `natural_earth/*.tmp`, so a
//!     leftover from a killed run never appears as an untracked file;
//!   * the fixed name self-heals — the next run's `File::create` truncates
//!     any leftover in place.
//!
//! The tradeoff: two *concurrent* writers of the same `path` would race on
//! the shared `.tmp` inode. That can't happen in this tool — the three worldview
//! bins use distinct paths and `just rasterize-geo` runs them sequentially.
//! If same-path parallelism is ever added, switch to random temp names (the
//! `tempfile` crate, whose `persist` is also cross-device-safe) rather than
//! reintroducing a lock.
//!
//! `tmp` must be a sibling of `path` (same directory): `std::fs::rename` is
//! not cross-device.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `path` with `suffix` appended, e.g. `geo_data.bin` -> `geo_data.bin.tmp`.
/// Uses `OsString::push` (append), NOT `Path::with_extension`, which would
/// *replace* the extension (`geo_data.tmp`) and break both the `*.bin.tmp`
/// gitignore glob and the sibling-tmp convention.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Atomically replace `path` with whatever `write` streams into the temp
/// file. On error the partial `<path>.tmp` is left behind and reclaimed by
/// the next run's `File::create`. The `fsync` is belt-and-suspenders for a
/// regenerable artifact; the parent-dir fsync is deliberately skipped.
pub fn write_atomically_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    let tmp = sibling(path, ".tmp");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    {
        let mut f = File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        write(&mut f).with_context(|| format!("writing {}", tmp.display()))?;
        f.sync_all()
            .with_context(|| format!("fsync {}", tmp.display()))?;
    }
    // Windows: rename over an existing file fails. Remove first.
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} → {}", tmp.display(), path.display()))
}

/// Convenience wrapper: atomically write an in-memory buffer to `path`.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomically_with(path, |f| {
        f.write_all(bytes)?;
        Ok(())
    })
}
