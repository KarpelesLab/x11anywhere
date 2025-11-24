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

    // Add Swift toolchain library path to rpath
    // This ensures the Swift runtime libraries can be found at runtime
    if let Ok(output) = Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
    {
        if output.status.success() {
            if let Ok(sdk_path) = String::from_utf8(output.stdout) {
                let sdk_path = sdk_path.trim();
                let swift_lib_path = format!("{}/usr/lib/swift", sdk_path);
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", swift_lib_path);
            }
        }
    }

    // Also add the toolchain's Swift library path
    if let Ok(output) = Command::new("xcrun")
        .args(["--find", "swift"])
        .output()
    {
        if output.status.success() {
            if let Ok(swift_path) = String::from_utf8(output.stdout) {
                let swift_path = swift_path.trim();
                // Get the directory containing swift binary, then go up to get the toolchain lib
                if let Some(swift_dir) = PathBuf::from(swift_path).parent() {
                    if let Some(toolchain_dir) = swift_dir.parent() {
                        let lib_path = toolchain_dir.join("lib").join("swift").join("macosx");
                        if lib_path.exists() {
                            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
                        }
                    }
                }
            }
        }
    }
}
