use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::Result;

/// Removes incomplete exports after a write or flush error, once the file is closed.
pub(crate) fn write_export_file(
    path: impl AsRef<Path>,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<()>,
) -> Result<()> {
    let path = path.as_ref();
    let mut file = BufWriter::new(File::create(path)?);
    let result = write(&mut file).and_then(|()| file.flush().map_err(Into::into));
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}
