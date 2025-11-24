//! Resource tracking for X11 clients
//!
//! This module provides common resource tracking that can be used across all
//! backends (X11, Windows, macOS). It tracks all resources created by a client
//! and generates cleanup requests when the client disconnects.

use crate::protocol::*;
use std::collections::{HashMap, HashSet};

/// Tracks all resources allocated by X11 clients
#[derive(Debug, Default)]
pub struct ResourceTracker {
    /// Windows created by each client
    windows: HashMap<u32, HashSet<XID>>,

    /// Pixmaps created by each client
    pixmaps: HashMap<u32, HashSet<XID>>,

    /// Graphics contexts created by each client
    gcs: HashMap<u32, HashSet<XID>>,

    /// Fonts opened by each client
    fonts: HashMap<u32, HashSet<u32>>,

    /// Cursors created by each client
    cursors: HashMap<u32, HashSet<u32>>,

    /// Colormaps created by each client
    colormaps: HashMap<u32, HashSet<u32>>,

    /// Atoms created by each client (for cleanup tracking)
    atoms: HashMap<u32, HashSet<Atom>>,

    /// Next client ID to assign
    next_client_id: u32,
}

impl ResourceTracker {
    /// Create a new resource tracker
    pub fn new() -> Self {
        ResourceTracker {
            windows: HashMap::new(),
            pixmaps: HashMap::new(),
            gcs: HashMap::new(),
            fonts: HashMap::new(),
            cursors: HashMap::new(),
            colormaps: HashMap::new(),
            atoms: HashMap::new(),
            next_client_id: 1,
        }
    }

    /// Register a new client and return its ID
    pub fn register_client(&mut self) -> u32 {
        let client_id = self.next_client_id;
        self.next_client_id += 1;

        // Initialize empty resource sets for this client
        self.windows.insert(client_id, HashSet::new());
        self.pixmaps.insert(client_id, HashSet::new());
        self.gcs.insert(client_id, HashSet::new());
        self.fonts.insert(client_id, HashSet::new());
        self.cursors.insert(client_id, HashSet::new());
        self.colormaps.insert(client_id, HashSet::new());
        self.atoms.insert(client_id, HashSet::new());

        client_id
    }

    /// Unregister a client (called on disconnect)
    pub fn unregister_client(&mut self, client_id: u32) -> Vec<CleanupRequest> {
        let mut cleanup_requests = Vec::new();

        // Generate cleanup requests for all resources
        if let Some(windows) = self.windows.remove(&client_id) {
            for xid in windows {
                cleanup_requests.push(CleanupRequest::DestroyWindow(Window(xid)));
            }
        }

        if let Some(pixmaps) = self.pixmaps.remove(&client_id) {
            for xid in pixmaps {
                cleanup_requests.push(CleanupRequest::FreePixmap(Pixmap::new(xid.0)));
            }
        }

        if let Some(gcs) = self.gcs.remove(&client_id) {
            for xid in gcs {
                cleanup_requests.push(CleanupRequest::FreeGC(GContext::new(xid.0)));
            }
        }

        if let Some(fonts) = self.fonts.remove(&client_id) {
            for font_id in fonts {
                cleanup_requests.push(CleanupRequest::CloseFont(font_id));
            }
        }

        if let Some(cursors) = self.cursors.remove(&client_id) {
            for cursor_id in cursors {
                cleanup_requests.push(CleanupRequest::FreeCursor(cursor_id));
            }
        }

        if let Some(colormaps) = self.colormaps.remove(&client_id) {
            for colormap_id in colormaps {
                cleanup_requests.push(CleanupRequest::FreeColormap(Colormap::new(colormap_id)));
            }
        }

        self.atoms.remove(&client_id);

        cleanup_requests
    }

    /// Track resource creation from a request
    pub fn track_request(&mut self, client_id: u32, request: &Request) {
        match request {
            Request::CreateWindow(req) => {
                if let Some(windows) = self.windows.get_mut(&client_id) {
                    windows.insert(req.wid.id());
                }
            }
            Request::CreatePixmap(req) => {
                if let Some(pixmaps) = self.pixmaps.get_mut(&client_id) {
                    pixmaps.insert(req.pid.id());
                }
            }
            Request::CreateGC(req) => {
                if let Some(gcs) = self.gcs.get_mut(&client_id) {
                    gcs.insert(req.cid.id());
                }
            }
            Request::OpenFont(req) => {
                if let Some(fonts) = self.fonts.get_mut(&client_id) {
                    fonts.insert(req.fid);
                }
            }
            Request::CreateGlyphCursor(req) => {
                if let Some(cursors) = self.cursors.get_mut(&client_id) {
                    cursors.insert(req.cid);
                }
            }
            Request::InternAtom(req) => {
                // Track atom requests (though atoms are global, we track which client used them)
                if !req.only_if_exists {
                    // Only track newly created atoms
                    if let Some(_atoms) = self.atoms.get_mut(&client_id) {
                        // We don't know the atom ID yet (comes in reply), but we track the request
                        // This is for informational purposes
                    }
                }
            }

            // Resource destruction
            Request::DestroyWindow(req) => {
                if let Some(windows) = self.windows.get_mut(&client_id) {
                    windows.remove(&req.window.id());
                }
            }
            Request::FreePixmap(req) => {
                if let Some(pixmaps) = self.pixmaps.get_mut(&client_id) {
                    pixmaps.remove(&req.pixmap.id());
                }
            }
            Request::FreeGC(req) => {
                if let Some(gcs) = self.gcs.get_mut(&client_id) {
                    gcs.remove(&req.gc.id());
                }
            }
            Request::CloseFont(req) => {
                if let Some(fonts) = self.fonts.get_mut(&client_id) {
                    fonts.remove(&req.font);
                }
            }

            _ => {
                // Other requests don't create/destroy tracked resources
            }
        }
    }

    /// Get all windows owned by a client
    pub fn get_client_windows(&self, client_id: u32) -> Option<&HashSet<XID>> {
        self.windows.get(&client_id)
    }

    /// Get all pixmaps owned by a client
    pub fn get_client_pixmaps(&self, client_id: u32) -> Option<&HashSet<XID>> {
        self.pixmaps.get(&client_id)
    }

    /// Check if a client owns a specific window
    pub fn client_owns_window(&self, client_id: u32, window: XID) -> bool {
        self.windows
            .get(&client_id)
            .map(|windows| windows.contains(&window))
            .unwrap_or(false)
    }

    /// Get resource counts for a client (for security limits)
    pub fn get_resource_counts(&self, client_id: u32) -> ResourceCounts {
        ResourceCounts {
            windows: self.windows.get(&client_id).map(|s| s.len()).unwrap_or(0),
            pixmaps: self.pixmaps.get(&client_id).map(|s| s.len()).unwrap_or(0),
            gcs: self.gcs.get(&client_id).map(|s| s.len()).unwrap_or(0),
            fonts: self.fonts.get(&client_id).map(|s| s.len()).unwrap_or(0),
            cursors: self.cursors.get(&client_id).map(|s| s.len()).unwrap_or(0),
            colormaps: self.colormaps.get(&client_id).map(|s| s.len()).unwrap_or(0),
        }
    }
}

/// Resource counts for a client
#[derive(Debug, Clone, Copy)]
pub struct ResourceCounts {
    pub windows: usize,
    pub pixmaps: usize,
    pub gcs: usize,
    pub fonts: usize,
    pub cursors: usize,
    pub colormaps: usize,
}

/// Cleanup request to send to backend when client disconnects
#[derive(Debug, Clone)]
pub enum CleanupRequest {
    DestroyWindow(Window),
    FreePixmap(Pixmap),
    FreeGC(GContext),
    CloseFont(u32),
    FreeCursor(u32),
    FreeColormap(Colormap),
}

impl CleanupRequest {
    /// Encode cleanup request to X11 protocol bytes
    pub fn encode(&self, byte_order: ByteOrder) -> Vec<u8> {
        match self {
            CleanupRequest::DestroyWindow(window) => {
                // DestroyWindow request: opcode=4, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 4; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], window.id().0, byte_order);
                buf
            }
            CleanupRequest::FreePixmap(pixmap) => {
                // FreePixmap request: opcode=54, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 54; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], pixmap.id().0, byte_order);
                buf
            }
            CleanupRequest::FreeGC(gc) => {
                // FreeGC request: opcode=60, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 60; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], gc.id().0, byte_order);
                buf
            }
            CleanupRequest::CloseFont(font) => {
                // CloseFont request: opcode=46, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 46; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], *font, byte_order);
                buf
            }
            CleanupRequest::FreeCursor(cursor) => {
                // FreeCursor request: opcode=95, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 95; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], *cursor, byte_order);
                buf
            }
            CleanupRequest::FreeColormap(colormap) => {
                // FreeColormap request: opcode=79, length=2
                let mut buf = vec![0u8; 8];
                buf[0] = 79; // opcode
                buf[1] = 0; // unused
                write_u16(&mut buf[2..4], 2, byte_order); // length
                write_u32(&mut buf[4..8], colormap.id().0, byte_order);
                buf
            }
        }
    }
}

// Helper functions for writing with correct byte order
fn write_u16(buf: &mut [u8], value: u16, byte_order: ByteOrder) {
    let bytes = match byte_order {
        ByteOrder::MSBFirst => value.to_be_bytes(),
        ByteOrder::LSBFirst => value.to_le_bytes(),
    };
    buf[0] = bytes[0];
    buf[1] = bytes[1];
}

fn write_u32(buf: &mut [u8], value: u32, byte_order: ByteOrder) {
    let bytes = match byte_order {
        ByteOrder::MSBFirst => value.to_be_bytes(),
        ByteOrder::LSBFirst => value.to_le_bytes(),
    };
    buf[0] = bytes[0];
    buf[1] = bytes[1];
    buf[2] = bytes[2];
    buf[3] = bytes[3];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_registration() {
        let mut tracker = ResourceTracker::new();
        let client1 = tracker.register_client();
        let client2 = tracker.register_client();

        assert_eq!(client1, 1);
        assert_eq!(client2, 2);
    }

    #[test]
    fn test_resource_tracking() {
        let mut tracker = ResourceTracker::new();
        let client_id = tracker.register_client();

        // Create a window
        let req = Request::CreateWindow(CreateWindowRequest {
            depth: 24,
            wid: Window::new(0x1000),
            parent: Window::new(1),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            border_width: 0,
            class: WindowClass::InputOutput,
            visual: VisualID::new(0),
            background_pixel: None,
            border_pixel: None,
            event_mask: None,
        });

        tracker.track_request(client_id, &req);

        assert!(tracker.client_owns_window(client_id, XID::new(0x1000)));
        assert_eq!(tracker.get_resource_counts(client_id).windows, 1);
    }

    #[test]
    fn test_cleanup_generation() {
        let mut tracker = ResourceTracker::new();
        let client_id = tracker.register_client();

        // Create resources
        tracker.track_request(
            client_id,
            &Request::CreateWindow(CreateWindowRequest {
                depth: 24,
                wid: Window::new(0x1000),
                parent: Window::new(1),
                x: 0,
                y: 0,
                width: 100,
                height: 100,
                border_width: 0,
                class: WindowClass::InputOutput,
                visual: VisualID::new(0),
                background_pixel: None,
                border_pixel: None,
                event_mask: None,
            }),
        );

        tracker.track_request(
            client_id,
            &Request::CreatePixmap(CreatePixmapRequest {
                depth: 24,
                pid: Pixmap::new(0x2000),
                drawable: Drawable::Window(Window::new(1)),
                width: 32,
                height: 32,
            }),
        );

        // Unregister client
        let cleanup = tracker.unregister_client(client_id);

        assert_eq!(cleanup.len(), 2);
    }
}
