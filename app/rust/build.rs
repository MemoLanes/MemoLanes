use std::env;
use std::path::{Path, PathBuf};
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

fn check_and_create_file(file_path: &str, warning_message: &str) {
    println!("cargo:rerun-if-changed={}", file_path);
    if fs::metadata(file_path).is_err() {
        let mut file = fs::File::create(file_path).unwrap();
        file.write_all(b"\n")
            .expect("failed to write to dummy file");

        file.flush().expect("failed to flush dummy file");
        println!("cargo:warning={}", warning_message);
    }
}

/// Checks and creates necessary dependency files if they do not exist
fn check_and_copy_yarn_file(src_path: &str, out_base_dir: &Path) {
    // Construct the destination path (preserving the original path structure)
    let src = Path::new(src_path);
    let dest = out_base_dir.join(src.file_name().expect("Failed to get file name"));

    // Automatically create the destination directory (including parent directories)
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|_| panic!("Failed to create directory: {:?}", parent));
    }

    // Dynamically handle the file: copy if it exists, create an empty file if it doesn't
    if src.exists() {
        fs::copy(src, &dest)
            .unwrap_or_else(|_| panic!("Failed to copy file: {:?} → {:?}", src, dest));
    } else {
        fs::write(&dest, "").unwrap_or_else(|_| panic!("Failed to create empty file: {:?}", dest));
    }

    // Set file watch
    println!("cargo:rerun-if-changed={}", src_path);
}

/// Generates a constant for the Mapbox token from environment variables or .env file
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
    // Generate Mapbox token constant
    generate_mapbox_token_const();

    // Trigger rebuild if .git directory changes
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

    // Generate protobuf files
    println!("cargo:rerun-if-changed=src/protos/journey.proto");
    println!("cargo:rerun-if-changed=src/protos/archive.proto");
    protobuf_codegen::Codegen::new()
        .pure()
        .out_dir("src/protos")
        .include("src/protos")
        .input("src/protos/journey.proto")
        .input("src/protos/archive.proto")
        .run_from_script();

    // Check and create necessary dependency files
    check_and_create_file(
        "src/frb_generated.rs",
        "`frb_generated.rs` is not found, generating a dummy file. If you are working on flutter, you need to run `flutter_rust_bridge_codegen generate` to get a real one."
    );

    // List of files to be embedded (wildcards need to be expanded manually)
    let files = [
        "../journey_kernel/dist/index.html",
        "../journey_kernel/dist/bundle.js",
        "../journey_kernel/dist/journey_kernel_bg.wasm",
    ];

    // Create a dedicated output directory (inside Cargo's temporary directory)
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("journey_kernel");

    // Process all files in batch
    for file in &files {
        check_and_copy_yarn_file(file, &out_dir);
    }

    // Setup workaround for x86_64 Android
    setup_x86_64_android_workaround();
}
