use std::{fs, io::Write};
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

    println!("cargo:rerun-if-changed=src/frb_generated.rs");
    if fs::metadata("src/frb_generated.rs").is_err() {
        fs::File::create("src/frb_generated.rs")
            .unwrap()
            .flush()
            .expect("failed to create dummpy frb_generated.rs");
        println!(
            "cargo:warning=`frb_generated.rs` is not found, generating a \
        dummpy file. If you are working on flutter, you need to run \
        `flutter_rust_bridge_codegen generate` to get a real one."
        );
    }
}
