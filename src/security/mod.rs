//! Security and isolation
//!
//! This module provides security features to protect the host system and
//! isolate X11 clients from each other.


// Allow dead code for now - skeleton implementation not yet integrated
#![allow(dead_code)]

/// Security policy configuration
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Isolate window trees - clients can only see their own windows
    pub window_isolation: bool,

    /// Allow access to global selections (clipboard)
    pub allow_global_selections: bool,

    /// Allow synthetic events (SendEvent from external clients)
    pub allow_synthetic_events: bool,

    /// Allow keyboard grabs
    pub allow_keyboard_grabs: bool,

    /// Allow pointer grabs
    pub allow_pointer_grabs: bool,

    /// Allow screen capture operations (GetImage on non-owned windows)
    pub allow_screen_capture: bool,

    /// Maximum windows per client (0 = unlimited)
    pub max_windows_per_client: usize,

    /// Maximum pixmaps per client (0 = unlimited)
    pub max_pixmaps_per_client: usize,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        SecurityPolicy {
            window_isolation: true,
            allow_global_selections: false,
            allow_synthetic_events: false, // Block SendEvent from external clients
            allow_keyboard_grabs: false,
            allow_pointer_grabs: true,  // Allow for modal dialogs
            allow_screen_capture: true, // Allow GetImage
            max_windows_per_client: 1000,
            max_pixmaps_per_client: 1000,
        }
    }
}

impl SecurityPolicy {
    /// Create a permissive policy (for testing) - allows everything
    pub fn permissive() -> Self {
        SecurityPolicy {
            window_isolation: false,
            allow_global_selections: true,
            allow_synthetic_events: true, // Allow SendEvent
            allow_keyboard_grabs: true,
            allow_pointer_grabs: true,
            allow_screen_capture: true,
            max_windows_per_client: 0,
            max_pixmaps_per_client: 0,
        }
    }

    /// Create a strict policy (maximum security) - strictly limit to owned windows
    pub fn strict() -> Self {
        SecurityPolicy {
            window_isolation: true,
            allow_global_selections: false,
            allow_synthetic_events: false, // Block SendEvent
            allow_keyboard_grabs: false,
            allow_pointer_grabs: false,
            allow_screen_capture: false, // Block GetImage on non-owned windows
            max_windows_per_client: 100,
            max_pixmaps_per_client: 100,
        }
    }
}
