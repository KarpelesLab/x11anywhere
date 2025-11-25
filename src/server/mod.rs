//! Core X11 server implementation
//!
//! This module contains the main server logic, including window management,
//! resource tracking, event dispatching, and client session management.

// Allow dead code for now - skeleton implementation not yet integrated
#![allow(dead_code)]

mod client;
pub mod listener;

use crate::backend::{Backend, BackendGC, BackendWindow};
use crate::protocol::*;
use crate::resources::ResourceTracker;
use crate::security::SecurityPolicy;
use std::collections::HashMap;
use std::error::Error;

/// Extension information
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub major_opcode: u8,
    pub first_event: u8,
    pub first_error: u8,
}

/// The main X11 server
pub struct Server {
    /// The display backend
    backend: Box<dyn Backend>,

    /// Window mapping: X11 Window ID -> Backend Window
    windows: HashMap<Window, BackendWindow>,

    /// GC mapping: X11 GContext ID -> Backend GC
    gcs: HashMap<GContext, BackendGC>,

    /// Root window
    root_window: Window,

    /// Root backend window
    root_backend_window: Option<BackendWindow>,

    /// Next resource ID to allocate
    next_resource_id: u32,

    /// Atom name -> ID mapping
    atom_names: HashMap<String, Atom>,

    /// Atom ID -> name mapping (reverse lookup)
    atom_ids: HashMap<Atom, String>,

    /// Next atom ID to allocate (predefined atoms use 1-68)
    next_atom_id: u32,

    /// Extension name -> info mapping
    extensions: HashMap<String, ExtensionInfo>,

    /// Font ID -> name mapping
    fonts: HashMap<u32, String>,

    /// Resource tracker for all clients
    resource_tracker: ResourceTracker,

    /// Security policy
    security_policy: SecurityPolicy,
}

impl Server {
    /// Create a new server with the given backend
    pub fn new(mut backend: Box<dyn Backend>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Initialize backend
        backend.init()?;

        // Create root window
        let root_window = Window::new(1); // Root is always ID 1

        let mut server = Server {
            backend,
            windows: HashMap::new(),
            gcs: HashMap::new(),
            root_window,
            root_backend_window: None,
            next_resource_id: 0x200, // Start after reserved IDs
            atom_names: HashMap::new(),
            atom_ids: HashMap::new(),
            next_atom_id: 69, // Predefined atoms use 1-68
            extensions: HashMap::new(),
            fonts: HashMap::new(),
            resource_tracker: ResourceTracker::new(),
            security_policy: SecurityPolicy::default(),
        };

        // Register predefined atoms
        server.init_predefined_atoms();

        // Register common extensions
        server.init_extensions();

        Ok(server)
    }

    /// Initialize predefined atoms (from X11 protocol spec)
    fn init_predefined_atoms(&mut self) {
        // Most commonly used predefined atoms
        let predefined = vec![
            (1, "PRIMARY"),
            (2, "SECONDARY"),
            (3, "ARC"),
            (4, "ATOM"),
            (5, "BITMAP"),
            (6, "CARDINAL"),
            (7, "COLORMAP"),
            (8, "CURSOR"),
            (9, "CUT_BUFFER0"),
            (10, "CUT_BUFFER1"),
            (11, "CUT_BUFFER2"),
            (12, "CUT_BUFFER3"),
            (13, "CUT_BUFFER4"),
            (14, "CUT_BUFFER5"),
            (15, "CUT_BUFFER6"),
            (16, "CUT_BUFFER7"),
            (17, "DRAWABLE"),
            (18, "FONT"),
            (19, "INTEGER"),
            (20, "PIXMAP"),
            (21, "POINT"),
            (22, "RECTANGLE"),
            (23, "RESOURCE_MANAGER"),
            (24, "RGB_COLOR_MAP"),
            (25, "RGB_BEST_MAP"),
            (26, "RGB_BLUE_MAP"),
            (27, "RGB_DEFAULT_MAP"),
            (28, "RGB_GRAY_MAP"),
            (29, "RGB_GREEN_MAP"),
            (30, "RGB_RED_MAP"),
            (31, "STRING"),
            (32, "VISUALID"),
            (33, "WINDOW"),
            (34, "WM_COMMAND"),
            (35, "WM_HINTS"),
            (36, "WM_CLIENT_MACHINE"),
            (37, "WM_ICON_NAME"),
            (38, "WM_ICON_SIZE"),
            (39, "WM_NAME"),
            (40, "WM_NORMAL_HINTS"),
            (41, "WM_SIZE_HINTS"),
            (42, "WM_ZOOM_HINTS"),
            (43, "MIN_SPACE"),
            (44, "NORM_SPACE"),
            (45, "MAX_SPACE"),
            (46, "END_SPACE"),
            (47, "SUPERSCRIPT_X"),
            (48, "SUPERSCRIPT_Y"),
            (49, "SUBSCRIPT_X"),
            (50, "SUBSCRIPT_Y"),
            (51, "UNDERLINE_POSITION"),
            (52, "UNDERLINE_THICKNESS"),
            (53, "STRIKEOUT_ASCENT"),
            (54, "STRIKEOUT_DESCENT"),
            (55, "ITALIC_ANGLE"),
            (56, "X_HEIGHT"),
            (57, "QUAD_WIDTH"),
            (58, "WEIGHT"),
            (59, "POINT_SIZE"),
            (60, "RESOLUTION"),
            (61, "COPYRIGHT"),
            (62, "NOTICE"),
            (63, "FONT_NAME"),
            (64, "FAMILY_NAME"),
            (65, "FULL_NAME"),
            (66, "CAP_HEIGHT"),
            (67, "WM_CLASS"),
            (68, "WM_TRANSIENT_FOR"),
        ];

        for (id, name) in predefined {
            let atom = Atom::new(id);
            self.atom_names.insert(name.to_string(), atom);
            self.atom_ids.insert(atom, name.to_string());
        }
    }

    /// Get the root window
    pub fn root_window(&self) -> Window {
        self.root_window
    }

    /// Get screen info from the backend
    pub fn get_screen_info(&self) -> crate::backend::ScreenInfo {
        self.backend
            .get_screen_info()
            .unwrap_or(crate::backend::ScreenInfo {
                width: 1920,
                height: 1080,
                width_mm: 508,
                height_mm: 285,
                root_visual: VisualID::new(0x21),
                root_depth: 24,
                white_pixel: 0xffffff,
                black_pixel: 0x000000,
            })
    }

    /// Allocate a new resource ID
    pub fn allocate_id(&mut self) -> u32 {
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        id
    }

    /// Intern an atom (register a name)
    /// Returns existing atom if already registered, or creates a new one
    pub fn intern_atom(&mut self, name: &str, only_if_exists: bool) -> Option<Atom> {
        // Check if atom already exists
        if let Some(&atom) = self.atom_names.get(name) {
            return Some(atom);
        }

        // If only_if_exists is true, return None (atom 0)
        if only_if_exists {
            return None;
        }

        // Allocate new atom
        let atom_id = self.next_atom_id;
        self.next_atom_id += 1;
        let atom = Atom::new(atom_id);

        // Register in both directions
        self.atom_names.insert(name.to_string(), atom);
        self.atom_ids.insert(atom, name.to_string());

        Some(atom)
    }

    /// Get atom name by ID
    pub fn get_atom_name(&self, atom: Atom) -> Option<&str> {
        self.atom_ids.get(&atom).map(|s| s.as_str())
    }

    /// Initialize common X11 extensions
    fn init_extensions(&mut self) {
        // BIG-REQUESTS extension (allows requests larger than 256KB)
        self.extensions.insert(
            "BIG-REQUESTS".to_string(),
            ExtensionInfo {
                major_opcode: 133,
                first_event: 0,
                first_error: 0,
            },
        );

        // XKEYBOARD extension (advanced keyboard input)
        self.extensions.insert(
            "XKEYBOARD".to_string(),
            ExtensionInfo {
                major_opcode: 135,
                first_event: 85,
                first_error: 0,
            },
        );

        // RENDER extension (anti-aliased rendering)
        self.extensions.insert(
            "RENDER".to_string(),
            ExtensionInfo {
                major_opcode: 138,
                first_event: 140,
                first_error: 0,
            },
        );
    }

    /// Query extension by name
    pub fn query_extension(&self, name: &str) -> Option<ExtensionInfo> {
        self.extensions.get(name).cloned()
    }

    /// Register a new client and return its ID
    pub fn register_client(&mut self) -> u32 {
        self.resource_tracker.register_client()
    }

    /// Unregister a client and generate cleanup requests
    pub fn unregister_client(&mut self, client_id: u32) -> Vec<crate::resources::CleanupRequest> {
        self.resource_tracker.unregister_client(client_id)
    }

    /// Track a request for resource management
    pub fn track_request(&mut self, client_id: u32, request: &Request) {
        self.resource_tracker.track_request(client_id, request);
    }

    /// Get the security policy
    pub fn security_policy(&self) -> &SecurityPolicy {
        &self.security_policy
    }

    /// Set a new security policy
    pub fn set_security_policy(&mut self, policy: SecurityPolicy) {
        self.security_policy = policy;
    }

    /// Handle a request from a client with resource tracking and security checks
    pub fn handle_request(
        &mut self,
        client_id: u32,
        request: &Request,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Track the request for resource management
        self.resource_tracker.track_request(client_id, request);

        // Apply security policy checks
        self.check_security_policy(client_id, request)?;

        // Apply resource limits
        self.check_resource_limits(client_id)?;

        // TODO: Actually process the request with the backend
        // For now, this is just tracking and security - actual request handling
        // will be implemented by the backend-specific server implementations

        Ok(())
    }

    /// Check if a request violates the security policy
    fn check_security_policy(
        &self,
        _client_id: u32,
        _request: &Request,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // TODO: Implement security checks when more request types are parsed
        // This will check for:
        // - Screen capture (GetImage) if !allow_screen_capture
        // - Keyboard grabs (GrabKeyboard) if !allow_keyboard_grabs
        // - Pointer grabs (GrabPointer) if !allow_pointer_grabs
        // - Synthetic events (SendEvent) if !allow_synthetic_events
        //
        // For now, all requests are allowed since the parser doesn't support these yet

        Ok(())
    }

    /// Check if client has exceeded resource limits
    fn check_resource_limits(&self, client_id: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        let counts = self.resource_tracker.get_resource_counts(client_id);

        // Check window limit
        if self.security_policy.max_windows_per_client > 0
            && counts.windows >= self.security_policy.max_windows_per_client
        {
            return Err(format!(
                "Client {} exceeded maximum windows limit ({})",
                client_id, self.security_policy.max_windows_per_client
            )
            .into());
        }

        // Check pixmap limit
        if self.security_policy.max_pixmaps_per_client > 0
            && counts.pixmaps >= self.security_policy.max_pixmaps_per_client
        {
            return Err(format!(
                "Client {} exceeded maximum pixmaps limit ({})",
                client_id, self.security_policy.max_pixmaps_per_client
            )
            .into());
        }

        Ok(())
    }

    /// Cleanup resources when a client disconnects
    pub fn handle_client_disconnect(
        &mut self,
        client_id: u32,
    ) -> Vec<crate::resources::CleanupRequest> {
        self.unregister_client(client_id)
    }

    /// Create a window
    pub fn create_window(
        &mut self,
        window: Window,
        parent: Window,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        class: WindowClass,
        _visual: VisualID,
        background_pixel: Option<u32>,
        event_mask: u32,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Get parent backend window (root or existing window)
        let parent_backend = if parent == self.root_window {
            self.root_backend_window
        } else {
            self.windows.get(&parent).copied()
        };

        let params = crate::backend::WindowParams {
            parent: parent_backend,
            x,
            y,
            width,
            height,
            border_width,
            class,
            background_pixel,
            event_mask,
        };

        let backend_window = self.backend.create_window(params)?;
        self.windows.insert(window, backend_window);

        // Store root backend window if this is the root
        if window == self.root_window && self.root_backend_window.is_none() {
            self.root_backend_window = Some(backend_window);
        }

        Ok(())
    }

    /// Map a window (make it visible)
    pub fn map_window(&mut self, window: Window) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(&backend_window) = self.windows.get(&window) {
            self.backend.map_window(backend_window)?;
        }
        Ok(())
    }

    /// Create a graphics context
    pub fn create_gc(
        &mut self,
        gc: GContext,
        _drawable: Drawable,
        foreground: Option<u32>,
        background: Option<u32>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut backend_gc = crate::backend::BackendGC::default();

        if let Some(fg) = foreground {
            backend_gc.foreground = fg;
        }
        if let Some(bg) = background {
            backend_gc.background = bg;
        }

        self.gcs.insert(gc, backend_gc);
        Ok(())
    }

    /// Change GC attributes
    pub fn change_gc(
        &mut self,
        gc: GContext,
        foreground: Option<u32>,
        background: Option<u32>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(backend_gc) = self.gcs.get_mut(&gc) {
            if let Some(fg) = foreground {
                backend_gc.foreground = fg;
            }
            if let Some(bg) = background {
                backend_gc.background = bg;
            }
        }
        Ok(())
    }

    /// Fill rectangles
    pub fn fill_rectangles(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        rectangles: &[Rectangle],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::debug!(
            "fill_rectangles: drawable={:?}, gc={:?}, {} rectangles, windows={}, gcs={}",
            drawable,
            gc,
            rectangles.len(),
            self.windows.len(),
            self.gcs.len()
        );

        // Get backend GC
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => {
                log::error!("fill_rectangles: Invalid GC {:?}", gc);
                return Err("Invalid GC".into());
            }
        };

        // Get backend drawable
        let backend_drawable = match drawable {
            Drawable::Window(w) => match self.windows.get(&w) {
                Some(backend_window) => {
                    log::debug!("fill_rectangles: found backend window {:?}", backend_window);
                    crate::backend::BackendDrawable::Window(*backend_window)
                }
                None => {
                    log::error!(
                        "fill_rectangles: Invalid window {:?}, known windows: {:?}",
                        w,
                        self.windows.keys().collect::<Vec<_>>()
                    );
                    return Err("Invalid window".into());
                }
            },
            Drawable::Pixmap(p) => crate::backend::BackendDrawable::Pixmap(p.id().get() as usize),
        };

        log::debug!("fill_rectangles: calling backend.fill_rectangles");

        // Draw all rectangles
        self.backend
            .fill_rectangles(backend_drawable, backend_gc, rectangles)?;
        self.backend.flush()?;

        log::debug!("fill_rectangles: completed successfully");
        Ok(())
    }

    /// Draw points
    pub fn draw_points(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        points: &[Point],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .draw_points(backend_drawable, backend_gc, points)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Draw connected lines
    pub fn draw_lines(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        points: &[Point],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .draw_lines(backend_drawable, backend_gc, points)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Draw line segments
    pub fn draw_segments(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        segments: &[(i16, i16, i16, i16)],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .draw_segments(backend_drawable, backend_gc, segments)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Draw rectangles (outlines)
    pub fn draw_rectangles(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        rectangles: &[Rectangle],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .draw_rectangles(backend_drawable, backend_gc, rectangles)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Draw arcs (outlines)
    pub fn draw_arcs(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        arcs: &[Arc],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend.draw_arcs(backend_drawable, backend_gc, arcs)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Fill arcs
    pub fn fill_arcs(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        arcs: &[Arc],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend.fill_arcs(backend_drawable, backend_gc, arcs)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Fill polygon
    pub fn fill_polygon(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        points: &[Point],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .fill_polygon(backend_drawable, backend_gc, points)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Put image
    pub fn put_image(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        data: &[u8],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend.put_image(
            backend_drawable,
            backend_gc,
            width,
            height,
            dst_x,
            dst_y,
            depth,
            format,
            data,
        )?;
        self.backend.flush()?;
        Ok(())
    }

    /// Draw text
    pub fn draw_text(
        &mut self,
        drawable: Drawable,
        gc: GContext,
        x: i16,
        y: i16,
        text: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .draw_text(backend_drawable, backend_gc, x, y, text)?;
        self.backend.flush()?;
        Ok(())
    }

    /// Open a font
    pub fn open_font(&mut self, font_id: u32, font_name: &str) {
        log::debug!("Opening font: id=0x{:x}, name={}", font_id, font_name);
        self.fonts.insert(font_id, font_name.to_string());
    }

    /// Close a font
    pub fn close_font(&mut self, font_id: u32) {
        log::debug!("Closing font: id=0x{:x}", font_id);
        self.fonts.remove(&font_id);
    }

    /// Helper to get backend drawable from X11 drawable
    fn get_backend_drawable(
        &self,
        drawable: Drawable,
    ) -> Result<crate::backend::BackendDrawable, Box<dyn Error + Send + Sync>> {
        match drawable {
            Drawable::Window(w) => match self.windows.get(&w) {
                Some(backend_window) => {
                    Ok(crate::backend::BackendDrawable::Window(*backend_window))
                }
                None => Err("Invalid window".into()),
            },
            Drawable::Pixmap(p) => Ok(crate::backend::BackendDrawable::Pixmap(
                p.id().get() as usize
            )),
        }
    }
}
