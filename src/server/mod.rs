/// Core X11 server implementation
///
/// This module contains the main server logic, including window management,
/// resource tracking, event dispatching, and client session management.

use crate::protocol::*;
use crate::backend::{Backend, BackendWindow};
use std::collections::HashMap;
use std::error::Error;

/// The main X11 server
pub struct Server {
    /// The display backend
    backend: Box<dyn Backend>,

    /// Window mapping: X11 Window ID -> Backend Window
    windows: HashMap<Window, BackendWindow>,

    /// Root window
    root_window: Window,

    /// Next resource ID to allocate
    next_resource_id: u32,
}

impl Server {
    /// Create a new server with the given backend
    pub fn new(mut backend: Box<dyn Backend>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Initialize backend
        backend.init()?;

        // Create root window
        let root_window = Window::new(1); // Root is always ID 1

        Ok(Server {
            backend,
            windows: HashMap::new(),
            root_window,
            next_resource_id: 0x200, // Start after reserved IDs
        })
    }

    /// Get the root window
    pub fn root_window(&self) -> Window {
        self.root_window
    }

    /// Allocate a new resource ID
    pub fn allocate_id(&mut self) -> u32 {
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        id
    }
}
