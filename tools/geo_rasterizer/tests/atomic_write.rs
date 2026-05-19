use std::fs;

use geo_rasterizer::atomic_write::{acquire_lock, write_atomically};

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
fn acquire_lock_creates_lock_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    let guard = acquire_lock(&path).unwrap();
    assert!(
        guard.lock_path.exists(),
        "lock file should exist while held"
    );
    drop(guard); // explicit drop to confirm the API permits it
}
