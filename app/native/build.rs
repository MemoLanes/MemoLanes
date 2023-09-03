use std::process::Command;

// need to install:
// - `cargo install flutter_rust_bridge_codegen`
fn main() {

    println!("cargo:rerun-if-changed=src/protos/journey.proto");
    protobuf_codegen::Codegen::new()
    .pure()
    .out_dir("src/protos")
    .include("src/protos")
    .input("src/protos/journey.proto")
    .run_from_script();

    println!("cargo:rerun-if-changed=src/api.rs");
    let output = Command::new("flutter_rust_bridge_codegen")
    .current_dir("..")
    .output()
    .expect("Failed to execute binary");
    if !output.status.success() {
        panic!("{:?}", output)
    }
}