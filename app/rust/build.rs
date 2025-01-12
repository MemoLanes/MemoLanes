use cargo_toml::Manifest;
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

fn check_yarn_dependencies() {
    // Check if yarn is installed
    if Command::new("yarn").arg("--version").output().is_err() {
        panic!("yarn is not installed. Please install yarn first using `npm install -g yarn`");
    }

    // Check if node_modules exists in journey_kernel
    if !Path::new("../journey_kernel/node_modules").exists() {
        println!("cargo:warning=Installing yarn dependencies for journey_kernel...");
        let status = Command::new("yarn")
            .current_dir("../journey_kernel")
            .arg("install")
            .status()
            .expect("Failed to run yarn install");

        if !status.success() {
            panic!("Failed to install yarn dependencies");
        }
    }
}

fn build_journey_kernel_wasm() {
    println!("cargo:rerun-if-changed=../journey_kernel");
    println!("cargo:rerun-if-changed=.journey_kernel_version");

    // Read version from Cargo.toml
    let manifest = Manifest::from_path("../journey_kernel/Cargo.toml")
        .expect("Failed to read journey_kernel Cargo.toml");
    let current_version = manifest
        .package
        .unwrap()
        .version
        .get()
        .unwrap().clone();

    // Check if version lock exists and matches
    let version_lock_path = Path::new("./target/.journey_kernel_version");
    if let Ok(locked_version) = fs::read_to_string(&version_lock_path) {
        if locked_version.trim() == current_version {
            println!("cargo:warning=Skipping journey_kernel build - version {} matches", current_version);
            return;
        }
    }

    // Build using webpack through yarn
    let status = Command::new("yarn")
        .current_dir("../journey_kernel")
        .args(["build"])
        .status()
        .expect("Failed to execute webpack build command");

    if !status.success() {
        panic!("Failed to build journey_kernel WASM package");
    }

    // Update version lock after successful build
    fs::write(version_lock_path, current_version)
        .expect("Failed to write journey_kernel version lock");
}

fn generate_mapbox_token_const() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
    let env_path = Path::new(&manifest_dir).join("../.env");

    println!("cargo:rerun-if-changed={}", env_path.display());

    // Try to load from environment first
    let token = if let Ok(token) = env::var("MAPBOX-ACCESS-TOKEN") {
        token
    } else {
        // Fallback to reading .env file
        match fs::read_to_string(&env_path) {
            Ok(env_content) => env_content
                .lines()
                .find(|line| line.starts_with("MAPBOX-ACCESS-TOKEN="))
                .map(|line| line.split('=').nth(1).unwrap().trim().to_string())
                .unwrap_or_else(|| {
                    println!("cargo:warning=MAPBOX-ACCESS-TOKEN not found in .env file");
                    String::new()
                }),
            Err(_) => {
                println!(
                    "cargo:warning=.env file not found at {}",
                    env_path.display()
                );
                String::new()
            }
        }
    };

    println!("cargo:rustc-env=MAPBOX-ACCESS-TOKEN={}", token);
}

fn main() {
    check_yarn_dependencies();
    build_journey_kernel_wasm();
    generate_mapbox_token_const();

    // There are articles on internet suggest `.git/HEAD` is enough, which I
    // doubt.
    println!("cargo:rerun-if-changed=../../.git");
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .expect("Failed to execute command");

    let git_hash = std::str::from_utf8(&output.stdout).unwrap().trim();
    std::fs::write(
        "src/build_info.rs",
        format!("// This file is auto generated by `build.rs`\npub const SHORT_COMMIT_HASH: &str = \"{}\";\n", git_hash),
    )
    .unwrap();

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
