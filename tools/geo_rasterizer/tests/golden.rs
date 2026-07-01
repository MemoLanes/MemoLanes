use std::process::Command;

use geo_data_format::read_geo_data;

/// A shipped asset must declare its own worldview id; the runtime
/// (`Storage::set_geo_data`) rejects a bin whose declared id differs from the
/// worldview it is loaded as, so a build tagging the wrong id would be caught.
#[test]
fn emitted_asset_declares_its_worldview_id() {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("worldview.bin");
    let status = Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--worldview",
            "iso",
            "--countries",
            "tests/fixtures/synthetic.geojson",
            "--registry",
            "tests/fixtures/synthetic_registry.toml",
            "--output",
        ])
        .arg(&out)
        .status()
        .expect("run rasterizer");
    assert!(status.success());

    let data = read_geo_data(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(data.worldview_id, "iso");
}

#[test]
fn golden_output_is_byte_identical() {
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("synthetic.bin");
    // synthetic_registry.toml assigns continents id 0-2 and countries id 3-5,
    // deliberately mirroring the pre-registry positional allocation so this
    // golden binary stays byte-stable across the Task 3 → Task 5 transition.
    // Do not renumber those ids or the golden must be regenerated.
    let status = Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--worldview",
            "iso",
            "--countries",
            "tests/fixtures/synthetic.geojson",
            "--registry",
            "tests/fixtures/synthetic_registry.toml",
            "--output",
        ])
        .arg(&out)
        .status()
        .expect("run rasterizer");
    assert!(status.success());

    let actual = std::fs::read(&out).unwrap();
    let expected = std::fs::read("tests/fixtures/synthetic.bin").unwrap();
    if actual != expected {
        let diff_at = actual
            .iter()
            .zip(expected.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(actual.len().min(expected.len()));
        panic!(
            "golden mismatch at byte {diff_at} (actual len {}, expected len {}). \
             To accept the new output: `just regenerate-golden`.",
            actual.len(),
            expected.len()
        );
    }
}
