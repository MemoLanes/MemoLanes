use std::env;
use std::path::Path;
use std::process::Command;
use std::{fs, io::Write};

fn main() {
    // There are articles on internet suggest `.git/HEAD` is enough, which I
    // doubt.
    println!("cargo:rerun-if-changed=../../.git");
    let short_commit_hash = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    println!("cargo:rustc-env=SHORT_COMMIT_HASH={}", short_commit_hash);

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
