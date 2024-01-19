use std::fs;
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

    

    println!("cargo:rerun-if-changed=src/api/*");
    let frb_codegen_installed = Command::new("flutter_rust_bridge_codegen")
        .arg("--version")
        .output()
        .is_ok();
    // NOTE: We skip running the frb codegen if the codegen is not installed.
    if frb_codegen_installed && fs::metadata("src/frb_generated.rs").is_err() {
        println!("cargo:rustc-cfg=flutterbuild");
        let output = Command::new("flutter_rust_bridge_codegen")
            .arg("generate")
            .current_dir("..")
            .output()
            .expect("Failed to execute binary");
        if !output.status.success() {
            panic!("{:?}", output)
        }
    }else if fs::metadata("src/frb_generated.rs").is_ok() {
        println!("cargo:rustc-cfg=flutterbuild");
    }
}
