use std::process::Command;

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
            "--pov",
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
