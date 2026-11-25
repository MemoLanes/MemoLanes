//! Atomic publish: stream into an exclusively created sibling, fsync, then
//! replace the destination. Readers see either the old or new complete file.
//! Random `.geo-publish-*.tmp` names never reuse existing staging aliases;
//! gitignore covers residue from killed processes. Normal errors clean up only
//! the newly created temporary file. Parent-directory fsync is deliberately
//! skipped for these regenerable artifacts.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Atomically replace `path` with streamed bytes, preserving the previous
/// destination if writing or publication fails. The temporary file must be in
/// the destination directory so replacement stays on the same filesystem.
pub fn write_atomically_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".geo-publish-").suffix(".tmp");
    // Match File::create's normal umask-controlled mode, not tempfile's 0600.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut tmp = builder
        .tempfile_in(parent)
        .with_context(|| format!("creating temporary file for {}", path.display()))?;
    write(tmp.as_file_mut()).with_context(|| format!("writing {}", tmp.path().display()))?;
    tmp.as_file()
        .sync_all()
        .with_context(|| format!("fsync {}", tmp.path().display()))?;
    let temporary_path = tmp.path().to_owned();
    // persist replaces existing files on Windows too, without unlinking first.
    tmp.persist(path)
        // Drop the returned temporary file even while callers retain the error.
        .map_err(|error| error.error)
        .with_context(|| format!("renaming {} → {}", temporary_path.display(), path.display()))?;
    Ok(())
}

/// Convenience wrapper: atomically write an in-memory buffer to `path`.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomically_with(path, |f| {
        f.write_all(bytes)?;
        Ok(())
    })
}
