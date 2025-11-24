#![allow(dead_code)] // Allow dead code during development

/// X11Anywhere - A portable X11 server implementation
///
/// This library provides a modular X11 server that can run on multiple platforms
/// and connect to different display backends (X11, Wayland, macOS, Windows).

pub mod protocol;
pub mod backend;
pub mod server;
pub mod connection;
pub mod security;
pub mod resources;

pub use protocol::{Window, Pixmap, Drawable, GContext, Atom};
pub use backend::{Backend, BackendWindow, BackendDrawable};

/// Server version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Protocol version
pub const PROTOCOL_MAJOR: u16 = 11;
pub const PROTOCOL_MINOR: u16 = 0;
