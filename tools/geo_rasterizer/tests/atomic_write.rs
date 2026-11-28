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

#[test]
fn write_atomically_with_preserves_destination_on_partial_writer_error() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    fs::write(&path, b"previous complete output").unwrap();
    let error = write_atomically_with(&path, |file| {
        file.write_all(b"partial replacement")?;
        anyhow::bail!("writer failed")
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("writer failed"));
    assert_eq!(fs::read(&path).unwrap(), b"previous complete output");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn write_atomically_supports_relative_paths_and_creates_directories() {
    const CHILD: &str = "GEO_ATOMIC_WRITE_RELATIVE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        for name in ["bare.bin", "new/nested/out.bin"] {
            write_atomically(std::path::Path::new(name), b"relative").unwrap();
            assert_eq!(fs::read(name).unwrap(), b"relative");
        }
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "write_atomically_supports_relative_paths_and_creates_directories",
        ])
        .env(CHILD, "1")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
}

#[test]
fn write_atomically_cleans_up_after_failed_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("blocker"), b"old").unwrap();
    let error = write_atomically(&path, b"new").unwrap_err();
    assert!(format!("{error:#}").contains("renaming"));
    assert_eq!(fs::read(path.join("blocker")).unwrap(), b"old");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn write_atomically_uses_normal_creation_permissions() {
    use std::os::unix::fs::PermissionsExt;

    // Inherit the caller's umask; never change process-global state in tests.
    let dir = tempfile::tempdir().unwrap();
    let normal = dir.path().join("normal.bin");
    let published = dir.path().join("published.bin");
    fs::File::create(&normal).unwrap();
    write_atomically(&published, b"published").unwrap();
    assert_eq!(
        fs::metadata(&published).unwrap().permissions().mode() & 0o777,
        fs::metadata(&normal).unwrap().permissions().mode() & 0o777,
        "fresh publication must use the same umask-controlled mode as File::create"
    );
}
