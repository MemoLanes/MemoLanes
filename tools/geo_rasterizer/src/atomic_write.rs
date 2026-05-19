//! Atomic write via `.tmp` + rename, plus an advisory exclusive lock
//! to prevent two parallel rasterizer invocations from racing.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt;

pub struct LockedOutput {
    _lock: File,
    pub lock_path: PathBuf,
}

/// Acquire an advisory exclusive lock on `<path>.lock`. Drops automatically
/// when the returned guard goes out of scope. Blocks if another process holds it.
pub fn acquire_lock(path: &Path) -> Result<LockedOutput> {
    let mut lock_path = path.as_os_str().to_owned();
    lock_path.push(".lock");
    let lock_path = PathBuf::from(lock_path);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("opening lock {}", lock_path.display()))?;
    lock.lock_exclusive()
        .with_context(|| format!("locking {}", lock_path.display()))?;
    Ok(LockedOutput {
        _lock: lock,
        lock_path,
    })
}

/// Write `bytes` to `path` atomically: write to `<path>.tmp`, fsync, rename.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp_os = path.as_os_str().to_owned();
    tmp_os.push(".tmp");
    let tmp = PathBuf::from(tmp_os);
    {
        let mut f = File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // Windows: rename over an existing file fails. Remove first.
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} → {}", tmp.display(), path.display()))?;
    Ok(())
}
