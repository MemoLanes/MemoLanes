use std::fs;

use geo_rasterizer::atomic_write::{write_atomically, write_atomically_with};

#[test]
fn write_atomically_persists_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    write_atomically(&path, b"hello-world").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"hello-world");
}

#[test]
fn write_atomically_overwrites_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    fs::write(&path, b"old").unwrap();
    write_atomically(&path, b"new").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new");
}

#[test]
fn write_atomically_creates_no_tmp_residue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    write_atomically(&path, b"data").unwrap();
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(
        entries.len(),
        1,
        "only out.bin should remain, no .tmp leftover"
    );
}

#[test]
fn write_atomically_with_streams_in_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    write_atomically_with(&path, |f| {
        use std::io::Write;
        f.write_all(b"chunk-a")?;
        f.write_all(b"chunk-b")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"chunk-achunk-b");
}
