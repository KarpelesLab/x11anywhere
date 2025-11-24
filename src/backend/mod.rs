//! Backend implementations
//!
//! This module contains the backend trait and implementations for different
//! display systems (X11, Wayland, macOS, Windows).

// Allow dead code for now - these are skeleton implementations not yet integrated
#![allow(dead_code)]

mod r#trait;
pub use r#trait::*;

// Backend implementations (feature-gated)

#[cfg(all(feature = "backend-x11", target_family = "unix"))]
pub mod x11;

// #[cfg(all(feature = "backend-wayland", target_os = "linux"))]
// pub mod wayland;

#[cfg(all(feature = "backend-macos", target_os = "macos"))]
pub mod macos;

#[cfg(all(feature = "backend-windows", target_os = "windows"))]
pub mod windows;

/// Get available backend names (features enabled + platform compatible)
pub fn available_backends() -> Vec<&'static str> {
    vec![
        // X11 backend is available on Unix systems when feature is enabled
        #[cfg(all(feature = "backend-x11", target_family = "unix"))]
        "x11",
        // Wayland backend is available on Linux when feature is enabled
        #[cfg(all(feature = "backend-wayland", target_os = "linux"))]
        "wayland",
        // macOS backend is available on macOS when feature is enabled
        #[cfg(all(feature = "backend-macos", target_os = "macos"))]
        "macos",
        // Windows backend is available on Windows when feature is enabled
        #[cfg(all(feature = "backend-windows", target_os = "windows"))]
        "windows",
    ]
}
