use std::fs;

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

    if fs::metadata("src/frb_generated.rs").is_ok() {
        println!("cargo:rustc-cfg=flutterbuild");
    }
}
