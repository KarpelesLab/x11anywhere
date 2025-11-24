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

    // Get the actual target architecture from Cargo (not host architecture)
    let target = env::var("TARGET").unwrap();
    let (swift_arch, swift_triple) = if target.contains("aarch64") {
        ("arm64", "arm64-apple-macosx")
    } else if target.contains("x86_64") {
        ("x86_64", "x86_64-apple-macosx")
    } else {
        panic!("Unsupported target architecture: {}", target);
    };

    let swift_dir = PathBuf::from(&manifest_dir).join("swift");
    let build_config = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    // Get SDK path for proper cross-compilation
    let sdk_path = if let Ok(output) = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
    {
        if output.status.success() {
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Build the Swift package for the target architecture
    println!("cargo:warning=Building Swift backend for {}...", swift_arch);
    let mut swift_build = Command::new("swift");
    swift_build.args([
        "build",
        "-c",
        build_config,
        "--arch",
        swift_arch,
        "--package-path",
        swift_dir.to_str().unwrap(),
    ]);

    // Add SDK and target triple for proper cross-compilation
    if !sdk_path.is_empty() {
        swift_build.args(["--sdk", &sdk_path]);
        // Set explicit target triple for cross-compilation
        let target_triple = format!("{}-apple-macosx", swift_arch);
        swift_build.args(["--triple", &target_triple]);
    }

    let status = swift_build.status().expect("Failed to execute swift build");

    if !status.success() {
        panic!("Swift build failed");
    }

    // Link the Swift library from the correct architecture directory
    let swift_build_dir = swift_dir
        .join(".build")
        .join(swift_triple)
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

    // Find and add Swift toolchain library paths for the target architecture
    if let Ok(output) = Command::new("xcrun").args(["--find", "swift"]).output() {
        if output.status.success() {
            if let Ok(swift_path) = String::from_utf8(output.stdout) {
                let swift_path = swift_path.trim();
                if let Some(swift_dir) = PathBuf::from(swift_path).parent() {
                    if let Some(toolchain_dir) = swift_dir.parent() {
                        let lib_path = toolchain_dir.join("lib").join("swift").join("macosx");
                        if lib_path.exists() {
                            // Add as library search path for linker
                            println!("cargo:rustc-link-search=native={}", lib_path.display());
                        }
                    }
                }
            }
        }
    }

    // Also try SDK path for Swift libraries
    if !sdk_path.is_empty() {
        let sdk_swift_lib = format!("{}/usr/lib/swift", sdk_path);
        let sdk_swift_path = PathBuf::from(&sdk_swift_lib);
        if sdk_swift_path.exists() {
            println!("cargo:rustc-link-search=native={}", sdk_swift_lib);
        }
    }

    // Link Swift runtime
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=dylib=swiftFoundation");
    println!("cargo:rustc-link-lib=dylib=swiftCoreGraphics");
    println!("cargo:rustc-link-lib=dylib=swiftAppKit");

    // Add Swift toolchain library path to rpath
    // This ensures the Swift runtime libraries can be found at runtime
    if !sdk_path.is_empty() {
        let swift_lib_path = format!("{}/usr/lib/swift", sdk_path);
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", swift_lib_path);
    }

    // Also add the toolchain's Swift library path to rpath
    if let Ok(output) = Command::new("xcrun").args(["--find", "swift"]).output() {
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

    // Add additional fallback rpaths that macOS searches
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
}
