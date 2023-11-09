use std::process::Command;

// need to install:
// - `cargo install flutter_rust_bridge_codegen`
fn main() {
    println!("cargo:rerun-if-changed=src/protos/journey.proto");
    println!("cargo:rerun-if-changed=src/protos/archive.proto");
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir("src/protos")
        .include("src/protos")
        .input("src/protos/journey.proto")
        .input("src/protos/archive.proto")
        .run_from_script();

    println!("cargo:rerun-if-changed=src/api.rs");
    let frb_codegen_installed = Command::new("flutter_rust_bridge_codegen")
        .arg("--version")
        .output()
        .is_ok();
    // NOTE: We skip running the frb codegen if the codegen is not installed.
    // This is mostly fine because we checked-in generated code in git.
    // Personally I don't like this idea, but by doing this we:
    // 1. People that worked on rust only don't need to install flutter +
    // the codegen.
    // 2. Make rust only github action faster.
    // 3. Avoid certain issues: mostly the versioning story of
    // `flutter_rust_bridge_codegen` is a little weird. And there is one more
    // issue with the rust cache on github action that I don't fully understand.
    // See: https://github.com/CaviarChen/ProjectDV/pull/22
    if frb_codegen_installed {
        let output = Command::new("flutter_rust_bridge_codegen")
            .current_dir("..")
            .output()
            .expect("Failed to execute binary");
        if !output.status.success() {
            panic!("{:?}", output)
        }
    } else {
        println!("cargo:warning=`flutter_rust_bridge_codegen` is not installed, skipping running the codegen.");
    }
}
