use std::env;
use std::path::Path;
use std::process::Command;
use std::{fs, io::Write};

/// Adds a temporary workaround for an issue with the Rust compiler and Android
/// in x86_64 devices: https://github.com/rust-lang/rust/issues/109717.
/// The workaround comes from: https://github.com/mozilla/application-services/pull/5442
fn setup_x86_64_android_workaround() {
    const DEFAULT_CLANG_VERSION: &str = "17";
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
        let linux_x86_64_lib_dir = format!(
            "toolchains/llvm/prebuilt/{build_os}-x86_64/lib/clang/{clang_version}/lib/linux/"
        );
        let linkpath = format!("{android_ndk_home}/{linux_x86_64_lib_dir}");
        if Path::new(&linkpath).exists() {
            println!("cargo:rustc-link-search={android_ndk_home}/{linux_x86_64_lib_dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.builtins-x86_64-android");
        } else {
            panic!("Path {linkpath} not exists");
        }
    }
}

fn check_wasm_pack_installed() {
    if Command::new("wasm-pack").arg("--version").output().is_err() {
        panic!("wasm-pack is not installed. Please install it by running `cargo install wasm-pack`.");
    }
}

fn build_journey_kernel_wasm() {
    println!("cargo:rerun-if-changed=../journey_kernel");
    
    let status = Command::new("wasm-pack")
        .current_dir("../journey_kernel")
        .args(&["build", "--target", "web", "--features", "wasm", "--no-default-features"])
        .status()
        .expect("Failed to execute wasm-pack command");

    if !status.success() {
        panic!("Failed to build journey_kernel WASM package");
    }
}

fn main() {
    check_wasm_pack_installed();
    build_journey_kernel_wasm();

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

    setup_x86_64_android_workaround();
}
