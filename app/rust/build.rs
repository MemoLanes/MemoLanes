use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io::Write};

/// Adds a temporary workaround for an issue with the Rust compiler and Android
/// in x86_64 devices: https://github.com/rust-lang/rust/issues/109717.
/// The workaround comes from: https://github.com/mozilla/application-services/pull/5442
fn setup_x86_64_android_workaround() {
    const DEFAULT_CLANG_VERSION: &str = "18";
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    let build_os = match env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        "windows" => "windows",
        _ => panic!(
            "Unsupported OS. You must use either Linux, MacOS or Windows to build the crate."
        ),
    };
    if target_arch == "x86_64" && target_os == "android" && build_os == "windows" {
        let android_ndk_home = env::var("ANDROID_NDK_HOME").expect("ANDROID_NDK_HOME not set");
        let clang_version =
            env::var("NDK_CLANG_VERSION").unwrap_or_else(|_| DEFAULT_CLANG_VERSION.to_owned());
        
        let mut lib_path = PathBuf::from(&android_ndk_home);
        lib_path.push("toolchains");
        lib_path.push("llvm");
        lib_path.push("prebuilt");
        lib_path.push(format!("{build_os}-x86_64"));
        lib_path.push("lib");
        lib_path.push("clang");
        lib_path.push(&clang_version);
        lib_path.push("lib");
        lib_path.push("linux");

        if lib_path.exists() {
            println!("cargo:rustc-link-search={}", lib_path.display());
            println!("cargo:rustc-link-lib=static=clang_rt.builtins-x86_64-android");
        } else {
            panic!("Path {} does not exist", lib_path.display());
        }
    }
}

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

    let proto_dir = PathBuf::from("src").join("protos");
    let journey_proto = proto_dir.join("journey.proto");
    let archive_proto = proto_dir.join("archive.proto");

    println!("cargo:rerun-if-changed={}", journey_proto.display());
    println!("cargo:rerun-if-changed={}", archive_proto.display());
    
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir(&proto_dir)
        .include(&proto_dir)
        .input(journey_proto)
        .input(archive_proto)
        .run_from_script();

    let frb_generated = PathBuf::from("src").join("frb_generated.rs");
    println!("cargo:rerun-if-changed={}", frb_generated.display());
    
    if fs::metadata(&frb_generated).is_err() {
        fs::File::create(&frb_generated)
            .unwrap()
            .flush()
            .expect("failed to create dummy frb_generated.rs");
        println!(
            "cargo:warning=`frb_generated.rs` is not found, generating a \
        dummy file. If you are working on flutter, you need to run \
        `flutter_rust_bridge_codegen generate` to get a real one."
        );
    }

    setup_x86_64_android_workaround();
}
