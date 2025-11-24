// build.rs - Build script for x11anywhere
// Compiles the Swift backend on macOS

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only build Swift code on macOS when backend-macos feature is enabled
    if cfg!(target_os = "macos") && cfg!(feature = "backend-macos") {
        build_swift_backend();
    }
}

fn build_swift_backend() {
    println!("cargo:rerun-if-changed=swift/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();

    let swift_dir = PathBuf::from(&manifest_dir).join("swift");
    let build_config = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    // Build the Swift package
    println!("cargo:warning=Building Swift backend...");
    let status = Command::new("swift")
        .args([
            "build",
            "-c",
            build_config,
            "--package-path",
            swift_dir.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to execute swift build");

    if !status.success() {
        panic!("Swift build failed");
    }

    // Link the Swift library
    let swift_build_dir = swift_dir
        .join(".build")
        .join(if cfg!(target_arch = "aarch64") {
            "arm64-apple-macosx"
        } else {
            "x86_64-apple-macosx"
        })
        .join(build_config);

    println!(
        "cargo:rustc-link-search=native={}",
        swift_build_dir.display()
    );
    println!("cargo:rustc-link-lib=static=X11AnywhereBackend");

    // Link required macOS frameworks
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=AppKit");

    // Link Swift runtime
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=dylib=swiftFoundation");
    println!("cargo:rustc-link-lib=dylib=swiftCoreGraphics");
    println!("cargo:rustc-link-lib=dylib=swiftAppKit");
}
