//! Core X11 server implementation
//!
//! This module contains the main server logic, including window management,
//! resource tracking, event dispatching, and client session management.

// Allow dead code for now - skeleton implementation not yet integrated
#![allow(dead_code)]

mod client;
pub mod extensions;
pub mod listener;

use crate::backend::{Backend, BackendCursor, BackendGC, BackendWindow, StandardCursor};
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

/// Property value stored on a window
#[derive(Debug, Clone)]
pub struct PropertyValue {
    /// The type of the property (an atom)
    pub type_: Atom,
    /// The format (8, 16, or 32 bits per element)
    pub format: u8,
    /// The raw data
    pub data: Vec<u8>,
}

/// Selection ownership info
#[derive(Debug, Clone)]
pub struct SelectionInfo {
    /// The window that owns this selection
    pub owner: Window,
    /// Timestamp when ownership was acquired
    pub time: u32,
}

/// Font information for QueryFont replies
#[derive(Debug, Clone)]
pub struct FontInfo {
    /// Font name as opened
    pub name: String,
    /// Font ascent (pixels above baseline)
    pub ascent: i16,
    /// Font descent (pixels below baseline)
    pub descent: i16,
    /// Average character width
    pub char_width: i16,
    /// Minimum character code
    pub min_char: u16,
    /// Maximum character code
    pub max_char: u16,
}

/// RENDER extension Picture resource
#[derive(Debug, Clone)]
pub struct Picture {
    /// The drawable (window or pixmap) this picture is attached to
    pub drawable: u32,
    /// Picture format ID
    pub format: u32,
    /// Component alpha flag
    pub component_alpha: bool,
}

/// RENDER extension Solid Fill picture
#[derive(Debug, Clone)]
pub struct SolidFill {
    /// Red component (0-65535)
    pub red: u16,
    /// Green component (0-65535)
    pub green: u16,
    /// Blue component (0-65535)
    pub blue: u16,
    /// Alpha component (0-65535)
    pub alpha: u16,
}

// Re-export RenderTrapezoid from backend for use by extensions
pub use crate::backend::RenderTrapezoid;

/// Window metadata for event dispatching and geometry queries
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Window width in pixels
    pub width: u16,
    /// Window height in pixels
    pub height: u16,
    /// Window x position
    pub x: i16,
    /// Window y position
    pub y: i16,
    /// Border width
    pub border_width: u16,
    /// Event mask (which events this window is interested in)
    pub event_mask: u32,
    /// Parent window
    pub parent: Window,
}

/// Pixmap metadata for geometry queries
#[derive(Debug, Clone)]
pub struct PixmapInfo {
    /// Pixmap width in pixels
    pub width: u16,
    /// Pixmap height in pixels
    pub height: u16,
    /// Pixmap depth
    pub depth: u8,
}

impl FontInfo {
    /// Create FontInfo with default metrics for a given font name
    pub fn new(name: &str) -> Self {
        // Default metrics for a typical fixed-width font
        // These are reasonable defaults for most text rendering
        FontInfo {
            name: name.to_string(),
            ascent: 12,
            descent: 4,
            char_width: 8,
            min_char: 0,
            max_char: 255,
        }
    }
}

/// The main X11 server
pub struct Server {
    /// The display backend
    backend: Box<dyn Backend>,

    /// Window mapping: X11 Window ID -> Backend Window
    windows: HashMap<Window, BackendWindow>,

    /// Reverse window mapping: Backend Window -> X11 Window ID (for event routing)
    backend_to_x11: HashMap<BackendWindow, Window>,

    /// Window metadata: X11 Window ID -> WindowInfo
    window_info: HashMap<Window, WindowInfo>,

    /// GC mapping: X11 GContext ID -> Backend GC
    gcs: HashMap<GContext, BackendGC>,

    /// Pixmap mapping: X11 Pixmap ID -> Backend pixmap ID
    pixmaps: HashMap<u32, usize>,

    /// Pixmap dimensions: X11 Pixmap ID -> PixmapInfo
    pixmap_info: HashMap<u32, PixmapInfo>,

    /// RENDER Picture mapping: Picture ID -> Picture info
    pictures: HashMap<u32, Picture>,

    /// RENDER Solid Fill mapping: Picture ID -> SolidFill info
    solid_fills: HashMap<u32, SolidFill>,

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

    /// Font ID -> font info mapping
    fonts: HashMap<u32, FontInfo>,

    /// Window properties: Window -> (Property Atom -> PropertyValue)
    properties: HashMap<Window, HashMap<Atom, PropertyValue>>,

    /// Selection ownership: Selection Atom -> SelectionInfo
    selections: HashMap<Atom, SelectionInfo>,

    /// Resource tracker for all clients
    resource_tracker: ResourceTracker,

    /// Security policy
    security_policy: SecurityPolicy,

    /// Pending events per window: Window -> Vec<encoded 32-byte events>
    pending_events: HashMap<Window, Vec<Vec<u8>>>,

    /// Event sequence number counter
    event_sequence: u16,

    /// Currently focused window
    focused_window: Window,

    /// Focus revert-to mode (0=None, 1=PointerRoot, 2=Parent)
    focus_revert_to: u8,

    /// Cursor mapping: X11 cursor ID -> BackendCursor
    cursors: HashMap<u32, BackendCursor>,

    /// Last known pointer X position
    last_pointer_x: i16,

    /// Last known pointer Y position
    last_pointer_y: i16,

    /// Last known window the pointer is in
    last_pointer_window: Window,
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
            backend_to_x11: HashMap::new(),
            window_info: HashMap::new(),
            gcs: HashMap::new(),
            pixmaps: HashMap::new(),
            pixmap_info: HashMap::new(),
            pictures: HashMap::new(),
            solid_fills: HashMap::new(),
            root_window,
            root_backend_window: None,
            next_resource_id: 0x200, // Start after reserved IDs
            atom_names: HashMap::new(),
            atom_ids: HashMap::new(),
            next_atom_id: 69, // Predefined atoms use 1-68
            extensions: HashMap::new(),
            fonts: HashMap::new(),
            properties: HashMap::new(),
            selections: HashMap::new(),
            resource_tracker: ResourceTracker::new(),
            security_policy: SecurityPolicy::default(),
            pending_events: HashMap::new(),
            event_sequence: 0,
            focused_window: root_window,
            focus_revert_to: 1, // PointerRoot
            cursors: HashMap::new(),
            last_pointer_x: 0,
            last_pointer_y: 0,
            last_pointer_window: root_window,
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

    /// Change a property on a window
    ///
    /// mode: 0=Replace, 1=Prepend, 2=Append
    pub fn change_property(
        &mut self,
        window: Window,
        property: Atom,
        type_: Atom,
        format: u8,
        mode: u8,
        data: Vec<u8>,
    ) {
        let window_props = self.properties.entry(window).or_default();

        match mode {
            0 => {
                // Replace
                window_props.insert(
                    property,
                    PropertyValue {
                        type_,
                        format,
                        data,
                    },
                );
            }
            1 => {
                // Prepend
                if let Some(existing) = window_props.get_mut(&property) {
                    let mut new_data = data;
                    new_data.extend_from_slice(&existing.data);
                    existing.data = new_data;
                } else {
                    window_props.insert(
                        property,
                        PropertyValue {
                            type_,
                            format,
                            data,
                        },
                    );
                }
            }
            2 => {
                // Append
                if let Some(existing) = window_props.get_mut(&property) {
                    existing.data.extend_from_slice(&data);
                } else {
                    window_props.insert(
                        property,
                        PropertyValue {
                            type_,
                            format,
                            data,
                        },
                    );
                }
            }
            _ => {} // Invalid mode, ignore
        }
    }

    /// Get a property from a window
    ///
    /// Returns None if the property doesn't exist
    pub fn get_property(
        &self,
        window: Window,
        property: Atom,
        _type_: Option<Atom>,
        _long_offset: u32,
        _long_length: u32,
        _delete: bool,
    ) -> Option<&PropertyValue> {
        self.properties.get(&window)?.get(&property)
    }

    /// Delete a property from a window
    pub fn delete_property(&mut self, window: Window, property: Atom) {
        if let Some(window_props) = self.properties.get_mut(&window) {
            window_props.remove(&property);
        }
    }

    /// List all properties on a window
    pub fn list_properties(&self, window: Window) -> Vec<Atom> {
        self.properties
            .get(&window)
            .map(|props| props.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Set the owner of a selection
    ///
    /// If owner is Window(0), the selection owner is cleared.
    /// Returns the previous owner (if any) for sending SelectionClear events.
    pub fn set_selection_owner(
        &mut self,
        selection: Atom,
        owner: Window,
        time: u32,
    ) -> Option<Window> {
        // Check time - if time is older than current ownership, ignore
        if let Some(current) = self.selections.get(&selection) {
            if time != 0 && current.time != 0 && time < current.time {
                return None;
            }
        }

        let previous_owner = self.selections.get(&selection).map(|s| s.owner);

        if owner == Window::NONE {
            // Clear selection
            self.selections.remove(&selection);
        } else {
            // Set new owner
            self.selections
                .insert(selection, SelectionInfo { owner, time });
        }

        previous_owner
    }

    /// Get the current owner of a selection
    ///
    /// Returns Window(0) if no owner is set.
    pub fn get_selection_owner(&self, selection: Atom) -> Window {
        self.selections
            .get(&selection)
            .map(|s| s.owner)
            .unwrap_or(Window::new(0))
    }

    /// Initialize common X11 extensions
    fn init_extensions(&mut self) {
        // SHAPE extension (non-rectangular windows)
        self.extensions.insert(
            "SHAPE".to_string(),
            ExtensionInfo {
                major_opcode: 129,
                first_event: 64, // ShapeNotify
                first_error: 0,
            },
        );

        // MIT-SHM extension (shared memory)
        self.extensions.insert(
            "MIT-SHM".to_string(),
            ExtensionInfo {
                major_opcode: 130,
                first_event: 65, // ShmCompletion
                first_error: 128,
            },
        );

        // BIG-REQUESTS extension (allows requests larger than 256KB)
        self.extensions.insert(
            "BIG-REQUESTS".to_string(),
            ExtensionInfo {
                major_opcode: 133,
                first_event: 0,
                first_error: 0,
            },
        );

        // SYNC extension (synchronization primitives)
        self.extensions.insert(
            "SYNC".to_string(),
            ExtensionInfo {
                major_opcode: 134,
                first_event: 83, // CounterNotify, AlarmNotify
                first_error: 128,
            },
        );

        // XKEYBOARD extension (advanced keyboard input)
        self.extensions.insert(
            "XKEYBOARD".to_string(),
            ExtensionInfo {
                major_opcode: 135,
                first_event: 85,
                first_error: 137,
            },
        );

        // XFIXES extension (misc fixes and features)
        self.extensions.insert(
            "XFIXES".to_string(),
            ExtensionInfo {
                major_opcode: 138,
                first_event: 87, // SelectionNotify, CursorNotify
                first_error: 140,
            },
        );

        // RENDER extension (anti-aliased rendering)
        self.extensions.insert(
            "RENDER".to_string(),
            ExtensionInfo {
                major_opcode: 139,
                first_event: 0, // No events
                first_error: 142,
            },
        );

        // RANDR extension (screen configuration)
        self.extensions.insert(
            "RANDR".to_string(),
            ExtensionInfo {
                major_opcode: 140,
                first_event: 89, // RRScreenChangeNotify, etc.
                first_error: 147,
            },
        );

        // COMPOSITE extension (off-screen rendering)
        self.extensions.insert(
            "Composite".to_string(),
            ExtensionInfo {
                major_opcode: 142,
                first_event: 0, // No events
                first_error: 0, // No errors
            },
        );

        // DAMAGE extension (damage tracking)
        self.extensions.insert(
            "DAMAGE".to_string(),
            ExtensionInfo {
                major_opcode: 143,
                first_event: 91, // DamageNotify
                first_error: 152,
            },
        );
    }

    /// Query extension by name
    pub fn query_extension(&self, name: &str) -> Option<ExtensionInfo> {
        self.extensions.get(name).cloned()
    }

    /// List all registered extensions
    pub fn list_extensions(&self) -> Vec<String> {
        self.extensions.keys().cloned().collect()
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

    /// Track a window creation for cleanup when client disconnects
    pub fn track_window(&mut self, client_id: u32, window_id: u32) {
        self.resource_tracker.track_window(client_id, window_id);
    }

    /// Track a GC creation for cleanup when client disconnects
    pub fn track_gc(&mut self, client_id: u32, gc_id: u32) {
        self.resource_tracker.track_gc(client_id, gc_id);
    }

    /// Track a pixmap creation for cleanup when client disconnects
    pub fn track_pixmap(&mut self, client_id: u32, pixmap_id: u32) {
        self.resource_tracker.track_pixmap(client_id, pixmap_id);
    }

    /// Track a font opening for cleanup when client disconnects
    pub fn track_font(&mut self, client_id: u32, font_id: u32) {
        self.resource_tracker.track_font(client_id, font_id);
    }

    /// Track a cursor creation for cleanup when client disconnects
    pub fn track_cursor(&mut self, client_id: u32, cursor_id: u32) {
        self.resource_tracker.track_cursor(client_id, cursor_id);
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
    #[allow(clippy::too_many_arguments)]
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
        log::info!(
            "Created window 0x{:08x} -> backend {:?}",
            window.id().0,
            backend_window
        );
        self.windows.insert(window, backend_window);
        self.backend_to_x11.insert(backend_window, window);
        log::debug!("Windows map now has {} entries", self.windows.len());

        // Store window metadata for event dispatching and geometry queries
        self.window_info.insert(
            window,
            WindowInfo {
                width,
                height,
                x,
                y,
                border_width,
                event_mask,
                parent,
            },
        );

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

    /// Get window info (dimensions, event_mask, etc.)
    pub fn get_window_info(&self, window: Window) -> Option<&WindowInfo> {
        self.window_info.get(&window)
    }

    /// Change window attributes (event_mask, cursor, etc.)
    pub fn change_window_attributes(
        &mut self,
        window: Window,
        event_mask: Option<u32>,
        cursor: Option<u32>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update event mask if specified
        if let Some(mask) = event_mask {
            if let Some(info) = self.window_info.get_mut(&window) {
                info.event_mask = mask;
                log::debug!(
                    "Changed window 0x{:x} event_mask to 0x{:x}",
                    window.id().get(),
                    mask
                );
            }
        }

        // Update cursor if specified
        if let Some(cursor_id) = cursor {
            let backend_cursor = if cursor_id == 0 {
                BackendCursor::NONE
            } else if let Some(&bc) = self.cursors.get(&cursor_id) {
                bc
            } else {
                log::debug!(
                    "Window 0x{:x} cursor 0x{:x} not in cursor map, ignoring",
                    window.id().get(),
                    cursor_id
                );
                return Ok(());
            };

            if let Some(&backend_window) = self.windows.get(&window) {
                self.backend
                    .set_window_cursor(backend_window, backend_cursor)?;
                log::debug!(
                    "Set window 0x{:x} cursor to 0x{:x}",
                    window.id().get(),
                    cursor_id
                );
            }
        }

        Ok(())
    }

    /// Get all child windows of a parent window
    pub fn get_children(&self, parent: Window) -> Vec<Window> {
        self.window_info
            .iter()
            .filter(|(_, info)| info.parent == parent)
            .map(|(window, _)| *window)
            .collect()
    }

    /// Set the input focus to a window
    pub fn set_input_focus(&mut self, focus: Window, revert_to: u8) {
        log::debug!(
            "Setting input focus to 0x{:x}, revert_to={}",
            focus.id().get(),
            revert_to
        );
        self.focused_window = focus;
        self.focus_revert_to = revert_to;
    }

    /// Get the current input focus
    pub fn get_input_focus(&self) -> (Window, u8) {
        (self.focused_window, self.focus_revert_to)
    }

    /// Clear an area of a window
    pub fn clear_area(
        &mut self,
        window: Window,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(&backend_window) = self.windows.get(&window) {
            // If width/height are 0, use the window dimensions
            let (w, h) = if width == 0 && height == 0 {
                if let Some(info) = self.window_info.get(&window) {
                    (info.width, info.height)
                } else {
                    return Ok(());
                }
            } else {
                (width, height)
            };
            self.backend.clear_area(backend_window, x, y, w, h)?;
            self.backend.flush()?;
        }
        Ok(())
    }

    /// Free a graphics context
    pub fn free_gc(&mut self, gc: GContext) {
        if self.gcs.remove(&gc).is_some() {
            log::debug!("Freed GC 0x{:x}", gc.id().get());
        } else {
            log::warn!("FreeGC: GC 0x{:x} not found", gc.id().get());
        }
    }

    /// Create a cursor from a standard cursor shape and store it
    pub fn create_cursor_from_standard(
        &mut self,
        cursor_id: u32,
        shape: StandardCursor,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_cursor = self.backend.create_standard_cursor(shape)?;
        self.cursors.insert(cursor_id, backend_cursor);
        log::debug!(
            "Created cursor 0x{:x} -> backend {:?}",
            cursor_id,
            backend_cursor
        );
        Ok(())
    }

    /// Store a cursor with a given backend cursor handle
    pub fn create_cursor(&mut self, cursor_id: u32, backend_cursor: BackendCursor) {
        self.cursors.insert(cursor_id, backend_cursor);
    }

    /// Free a cursor
    pub fn free_cursor(&mut self, cursor_id: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(backend_cursor) = self.cursors.remove(&cursor_id) {
            self.backend.free_cursor(backend_cursor)?;
            log::debug!("Freed cursor 0x{:x}", cursor_id);
        }
        Ok(())
    }

    /// Set the cursor for a window using a cursor ID from the cursor map
    pub fn set_window_cursor(
        &mut self,
        window: Window,
        cursor_id: u32,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_cursor = if cursor_id == 0 {
            BackendCursor::NONE
        } else if let Some(&cursor) = self.cursors.get(&cursor_id) {
            cursor
        } else {
            log::warn!(
                "set_window_cursor: cursor 0x{:x} not found in cursor map",
                cursor_id
            );
            return Ok(());
        };

        if let Some(&backend_window) = self.windows.get(&window) {
            self.backend
                .set_window_cursor(backend_window, backend_cursor)?;
        }
        Ok(())
    }

    /// Translate coordinates from one window's coordinate space to another
    ///
    /// Returns (dst_x, dst_y, child_window) where child_window is the child
    /// of dst_window that contains the point, or Window::NONE if none.
    pub fn translate_coordinates(
        &self,
        src_window: Window,
        dst_window: Window,
        src_x: i16,
        src_y: i16,
    ) -> (i16, i16, Window) {
        // Convert src coords to root coords by walking up the parent chain
        let (root_x, root_y) = self.window_to_root_coords(src_window, src_x, src_y);

        // Convert root coords to dst coords by walking up dst parent chain
        let (dst_off_x, dst_off_y) = self.window_to_root_coords(dst_window, 0, 0);
        let dst_x = root_x - dst_off_x;
        let dst_y = root_y - dst_off_y;

        // Find child of dst_window that contains the point
        let child = self.find_child_at(dst_window, dst_x, dst_y);

        (dst_x, dst_y, child)
    }

    /// Convert window-local coordinates to root coordinates
    fn window_to_root_coords(&self, window: Window, x: i16, y: i16) -> (i16, i16) {
        let mut abs_x = x;
        let mut abs_y = y;
        let mut current = window;

        while current != self.root_window && current != Window::NONE {
            if let Some(info) = self.window_info.get(&current) {
                abs_x += info.x;
                abs_y += info.y;
                current = info.parent;
            } else {
                break;
            }
        }

        (abs_x, abs_y)
    }

    /// Find the child of parent_window that contains the point (x, y)
    fn find_child_at(&self, parent: Window, x: i16, y: i16) -> Window {
        for (win, info) in &self.window_info {
            if info.parent == parent
                && x >= info.x
                && y >= info.y
                && x < info.x + info.width as i16
                && y < info.y + info.height as i16
            {
                return *win;
            }
        }
        Window::NONE
    }

    /// Update the tracked pointer position
    pub fn update_pointer_position(&mut self, x: i16, y: i16, window: Window) {
        self.last_pointer_x = x;
        self.last_pointer_y = y;
        self.last_pointer_window = window;
    }

    /// Get the last known pointer position
    pub fn get_pointer_position(&self) -> (i16, i16, Window) {
        (
            self.last_pointer_x,
            self.last_pointer_y,
            self.last_pointer_window,
        )
    }

    /// Find a child window (or the window itself) that wants events with the given mask.
    /// This implements X11 event propagation - events go to the deepest child that wants them.
    /// Returns the window to deliver the event to and its event_mask.
    fn find_window_for_event(&self, window: Window, required_mask: u32) -> Option<(Window, u32)> {
        // First check if any child window wants this event (search children first for deeper propagation)
        let children = self.get_children(window);
        for child in children {
            if let Some(result) = self.find_window_for_event(child, required_mask) {
                return Some(result);
            }
        }

        // Then check if this window itself wants the event
        if let Some(info) = self.window_info.get(&window) {
            if info.event_mask & required_mask != 0 {
                return Some((window, info.event_mask));
            }
        }

        None
    }

    /// Unmap a window (hide it)
    pub fn unmap_window(&mut self, window: Window) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(&backend_window) = self.windows.get(&window) {
            self.backend.unmap_window(backend_window)?;
        }
        Ok(())
    }

    /// Destroy a window
    pub fn destroy_window(&mut self, window: Window) -> Result<(), Box<dyn Error + Send + Sync>> {
        log::debug!(
            "destroy_window called for window 0x{:08x}, windows map has {} entries",
            window.id().0,
            self.windows.len()
        );
        if let Some(&backend_window) = self.windows.get(&window) {
            log::info!(
                "Destroying window 0x{:08x} -> backend {:?}",
                window.id().0,
                backend_window
            );
            self.backend.destroy_window(backend_window)?;
            self.windows.remove(&window);
            self.backend_to_x11.remove(&backend_window);
            log::debug!("Window destroyed and removed from map");
        } else {
            log::warn!("Window 0x{:08x} not found in windows map!", window.id().0);
        }
        Ok(())
    }

    /// Reparent a window to a new parent
    pub fn reparent_window(
        &mut self,
        window: Window,
        parent: Window,
        x: i16,
        y: i16,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Update the window's parent in window_info
        if let Some(info) = self.window_info.get_mut(&window) {
            info.parent = parent;
            log::debug!(
                "Reparented window 0x{:x} to parent 0x{:x} at ({}, {})",
                window.id().get(),
                parent.id().get(),
                x,
                y
            );
        }
        // Note: Actual reparenting in the backend would require additional
        // backend support. For now, we track the logical parent relationship.
        // This is sufficient for most X11 clients that use reparenting for
        // window manager integration.
        Ok(())
    }

    /// Configure a window (resize/move)
    pub fn configure_window(
        &mut self,
        window: Window,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(&backend_window) = self.windows.get(&window) {
            let config = crate::backend::WindowConfig {
                x,
                y,
                width,
                height,
                border_width: None,
                stack_mode: None,
            };
            self.backend.configure_window(backend_window, config)?;

            // Update window_info with new geometry
            if let Some(info) = self.window_info.get_mut(&window) {
                if let Some(new_x) = x {
                    info.x = new_x;
                }
                if let Some(new_y) = y {
                    info.y = new_y;
                }
                if let Some(new_width) = width {
                    info.width = new_width;
                }
                if let Some(new_height) = height {
                    info.height = new_height;
                }
            }
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

    /// Copy GC attributes from one GC to another
    pub fn copy_gc(
        &mut self,
        src_gc: GContext,
        dst_gc: GContext,
        value_mask: u32,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Get the source GC values
        let src_values = if let Some(src) = self.gcs.get(&src_gc) {
            src.clone()
        } else {
            log::warn!("CopyGC: source GC 0x{:x} not found", src_gc.id().get());
            return Ok(());
        };

        // Apply masked values to destination GC
        if let Some(dst) = self.gcs.get_mut(&dst_gc) {
            // Copy attributes based on value_mask
            // Bit 0: function
            // Bit 1: plane-mask
            // Bit 2: foreground
            if value_mask & (1 << 2) != 0 {
                dst.foreground = src_values.foreground;
            }
            // Bit 3: background
            if value_mask & (1 << 3) != 0 {
                dst.background = src_values.background;
            }
            // Bit 4: line-width
            if value_mask & (1 << 4) != 0 {
                dst.line_width = src_values.line_width;
            }
            // Bit 5: line-style
            if value_mask & (1 << 5) != 0 {
                dst.line_style = src_values.line_style;
            }
            // Bit 6: cap-style
            if value_mask & (1 << 6) != 0 {
                dst.cap_style = src_values.cap_style;
            }
            // Bit 7: join-style
            if value_mask & (1 << 7) != 0 {
                dst.join_style = src_values.join_style;
            }
            // Bit 8: fill-style
            if value_mask & (1 << 8) != 0 {
                dst.fill_style = src_values.fill_style;
            }
            // Note: font (bit 14) is tracked separately in the server, not in BackendGC
            log::debug!(
                "CopyGC: copied attributes from 0x{:x} to 0x{:x} (mask=0x{:x})",
                src_gc.id().get(),
                dst_gc.id().get(),
                value_mask
            );
        } else {
            log::warn!("CopyGC: destination GC 0x{:x} not found", dst_gc.id().get());
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
            Some(gc) => {
                log::debug!(
                    "fill_rectangles: using foreground=0x{:08x} (R={}, G={}, B={})",
                    gc.foreground,
                    (gc.foreground >> 16) & 0xff,
                    (gc.foreground >> 8) & 0xff,
                    gc.foreground & 0xff
                );
                gc
            }
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
            Drawable::Pixmap(p) => {
                let pixmap_id = p.id().get();
                match self.pixmaps.get(&pixmap_id) {
                    Some(&backend_id) => {
                        log::debug!(
                            "fill_rectangles: mapped pixmap 0x{:x} -> backend {}",
                            pixmap_id,
                            backend_id
                        );
                        crate::backend::BackendDrawable::Pixmap(backend_id)
                    }
                    None => {
                        log::error!(
                            "fill_rectangles: Pixmap 0x{:x} not found in pixmaps, available: {:?}",
                            pixmap_id,
                            self.pixmaps.keys().collect::<Vec<_>>()
                        );
                        return Err(format!("Pixmap 0x{:x} not found", pixmap_id).into());
                    }
                }
            }
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
    #[allow(clippy::too_many_arguments)]
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

    /// Copy area from one drawable to another
    #[allow(clippy::too_many_arguments)]
    pub fn copy_area(
        &mut self,
        src_drawable: Drawable,
        dst_drawable: Drawable,
        gc: GContext,
        src_x: i16,
        src_y: i16,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_gc = match self.gcs.get(&gc) {
            Some(gc) => gc,
            None => return Err("Invalid GC".into()),
        };

        let backend_src = self.get_backend_drawable(src_drawable)?;
        let backend_dst = self.get_backend_drawable(dst_drawable)?;

        self.backend.copy_area(
            backend_src,
            backend_dst,
            backend_gc,
            src_x,
            src_y,
            width,
            height,
            dst_x,
            dst_y,
        )?;
        self.backend.flush()?;
        Ok(())
    }

    /// Get image data from a drawable
    #[allow(clippy::too_many_arguments)]
    pub fn get_image(
        &mut self,
        drawable: Drawable,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
        format: u8,
    ) -> Result<(u8, u32, Vec<u8>), Box<dyn Error + Send + Sync>> {
        let backend_drawable = self.get_backend_drawable(drawable)?;

        self.backend
            .get_image(backend_drawable, x, y, width, height, plane_mask, format)
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

        // Try to get real font metrics from the backend
        let font_info = if let Ok(Some(metrics)) = self.backend.query_font_metrics(font_name) {
            log::debug!(
                "Got font metrics from backend: ascent={}, descent={}, char_width={}",
                metrics.ascent,
                metrics.descent,
                metrics.char_width
            );
            FontInfo {
                name: font_name.to_string(),
                ascent: metrics.ascent,
                descent: metrics.descent,
                char_width: metrics.char_width as i16,
                min_char: 0,
                max_char: 255,
            }
        } else {
            // Fall back to default metrics
            FontInfo::new(font_name)
        };

        self.fonts.insert(font_id, font_info);
    }

    /// Close a font
    pub fn close_font(&mut self, font_id: u32) {
        log::debug!("Closing font: id=0x{:x}", font_id);
        self.fonts.remove(&font_id);
    }

    /// Query font information
    pub fn query_font(&self, font_id: u32) -> Option<&FontInfo> {
        self.fonts.get(&font_id)
    }

    /// List fonts matching a pattern
    /// Pattern uses X11 font pattern syntax: * matches any sequence, ? matches any char
    pub fn list_fonts(&self, pattern: &str, max_names: u16) -> Vec<String> {
        // Built-in fonts that we can report
        let available_fonts = vec![
            "fixed",
            "6x10",
            "6x12",
            "6x13",
            "7x13",
            "7x14",
            "8x13",
            "8x16",
            "9x15",
            "9x18",
            "10x20",
            "cursor",
            "-misc-fixed-medium-r-normal--13-120-75-75-c-80-iso8859-1",
            "-misc-fixed-medium-r-normal--15-140-75-75-c-90-iso8859-1",
            "-misc-fixed-bold-r-normal--13-120-75-75-c-80-iso8859-1",
            "-misc-fixed-bold-r-normal--15-140-75-75-c-90-iso8859-1",
        ];

        // Convert X11 glob pattern to simple matching
        let pattern_lower = pattern.to_lowercase();
        let is_wildcard_all = pattern == "*";

        let mut result: Vec<String> = available_fonts
            .into_iter()
            .filter(|font| {
                if is_wildcard_all {
                    true
                } else {
                    // Simple pattern matching: convert * to any sequence
                    let font_lower = font.to_lowercase();
                    if pattern_lower.contains('*') {
                        // Split pattern by * and check each part
                        let parts: Vec<&str> = pattern_lower.split('*').collect();
                        let mut pos = 0;
                        for (i, part) in parts.iter().enumerate() {
                            if part.is_empty() {
                                continue;
                            }
                            if let Some(found_pos) = font_lower[pos..].find(part) {
                                // First part must match at start
                                if i == 0 && found_pos != 0 {
                                    return false;
                                }
                                pos += found_pos + part.len();
                            } else {
                                return false;
                            }
                        }
                        true
                    } else {
                        font_lower.contains(&pattern_lower)
                    }
                }
            })
            .map(|s| s.to_string())
            .collect();

        // Limit to max_names
        result.truncate(max_names as usize);
        result
    }

    /// Convert RGB values (0-65535 range) to a pixel value
    /// For TrueColor, this packs the RGB into 0xRRGGBB format
    pub fn alloc_color(&self, red: u16, green: u16, blue: u16) -> u32 {
        // Convert from 16-bit to 8-bit per component
        let r = (red >> 8) as u32;
        let g = (green >> 8) as u32;
        let b = (blue >> 8) as u32;
        (r << 16) | (g << 8) | b
    }

    /// Look up a named color and return (pixel, exact_r, exact_g, exact_b, visual_r, visual_g, visual_b)
    /// Returns None if the color name is not found
    pub fn lookup_named_color(&self, name: &str) -> Option<(u32, u16, u16, u16, u16, u16, u16)> {
        // Standard X11 color names (subset of rgb.txt)
        let color_map: &[(&str, (u8, u8, u8))] = &[
            ("black", (0, 0, 0)),
            ("white", (255, 255, 255)),
            ("red", (255, 0, 0)),
            ("green", (0, 255, 0)),
            ("blue", (0, 0, 255)),
            ("yellow", (255, 255, 0)),
            ("cyan", (0, 255, 255)),
            ("magenta", (255, 0, 255)),
            ("gray", (128, 128, 128)),
            ("grey", (128, 128, 128)),
            ("darkgray", (64, 64, 64)),
            ("darkgrey", (64, 64, 64)),
            ("lightgray", (192, 192, 192)),
            ("lightgrey", (192, 192, 192)),
            ("orange", (255, 165, 0)),
            ("pink", (255, 192, 203)),
            ("brown", (165, 42, 42)),
            ("purple", (128, 0, 128)),
            ("navy", (0, 0, 128)),
            ("maroon", (128, 0, 0)),
            ("olive", (128, 128, 0)),
            ("teal", (0, 128, 128)),
            ("silver", (192, 192, 192)),
            ("gold", (255, 215, 0)),
            ("coral", (255, 127, 80)),
            ("salmon", (250, 128, 114)),
            ("tomato", (255, 99, 71)),
            ("firebrick", (178, 34, 34)),
            ("darkred", (139, 0, 0)),
            ("darkgreen", (0, 100, 0)),
            ("darkblue", (0, 0, 139)),
            ("lightblue", (173, 216, 230)),
            ("lightgreen", (144, 238, 144)),
            ("violet", (238, 130, 238)),
            ("indigo", (75, 0, 130)),
            ("wheat", (245, 222, 179)),
            ("tan", (210, 180, 140)),
            ("khaki", (240, 230, 140)),
            ("aqua", (0, 255, 255)),
            ("lime", (0, 255, 0)),
            ("ivory", (255, 255, 240)),
            ("snow", (255, 250, 250)),
            ("steelblue", (70, 130, 180)),
            ("royalblue", (65, 105, 225)),
            ("skyblue", (135, 206, 235)),
            ("turquoise", (64, 224, 208)),
            ("chartreuse", (127, 255, 0)),
            ("lawngreen", (124, 252, 0)),
            ("forestgreen", (34, 139, 34)),
            ("seagreen", (46, 139, 87)),
            ("springgreen", (0, 255, 127)),
            ("mintcream", (245, 255, 250)),
            ("honeydew", (240, 255, 240)),
            ("lavender", (230, 230, 250)),
            ("plum", (221, 160, 221)),
            ("orchid", (218, 112, 214)),
            ("hotpink", (255, 105, 180)),
            ("deeppink", (255, 20, 147)),
            ("mistyrose", (255, 228, 225)),
            ("peachpuff", (255, 218, 185)),
            ("papayawhip", (255, 239, 213)),
            ("lemonchiffon", (255, 250, 205)),
            ("beige", (245, 245, 220)),
            ("linen", (250, 240, 230)),
            ("oldlace", (253, 245, 230)),
            ("antiquewhite", (250, 235, 215)),
            ("bisque", (255, 228, 196)),
            ("blanchedalmond", (255, 235, 205)),
            ("moccasin", (255, 228, 181)),
            ("navajowhite", (255, 222, 173)),
        ];

        let name_lower = name.to_lowercase().replace(" ", "");

        for (color_name, (r, g, b)) in color_map {
            if *color_name == name_lower {
                // Convert 8-bit to 16-bit values
                let r16 = (*r as u16) << 8 | (*r as u16);
                let g16 = (*g as u16) << 8 | (*g as u16);
                let b16 = (*b as u16) << 8 | (*b as u16);

                // Pixel value for TrueColor
                let pixel = ((*r as u32) << 16) | ((*g as u32) << 8) | (*b as u32);

                return Some((pixel, r16, g16, b16, r16, g16, b16));
            }
        }

        None
    }

    /// Create a pixmap in the backend and register its ID
    pub fn create_pixmap(
        &mut self,
        pixmap_id: u32,
        width: u16,
        height: u16,
        depth: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let backend_id = self.backend.create_pixmap(width, height, depth)?;
        self.pixmaps.insert(pixmap_id, backend_id);
        self.pixmap_info.insert(
            pixmap_id,
            PixmapInfo {
                width,
                height,
                depth,
            },
        );
        log::debug!(
            "Created pixmap 0x{:x} -> backend {} ({}x{}, depth={})",
            pixmap_id,
            backend_id,
            width,
            height,
            depth
        );
        Ok(())
    }

    /// Free a pixmap from the backend and unregister its ID
    pub fn free_pixmap(&mut self, pixmap_id: u32) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(backend_id) = self.pixmaps.remove(&pixmap_id) {
            self.pixmap_info.remove(&pixmap_id);
            self.backend.free_pixmap(backend_id)?;
            log::debug!("Freed pixmap 0x{:x} (backend {})", pixmap_id, backend_id);
        }
        Ok(())
    }

    /// Get pixmap info (dimensions)
    pub fn get_pixmap_info(&self, pixmap_id: u32) -> Option<&PixmapInfo> {
        self.pixmap_info.get(&pixmap_id)
    }

    /// Resolve a drawable ID to a Drawable enum
    /// Checks if the ID is a known pixmap, otherwise assumes it's a window
    pub fn resolve_drawable(&self, drawable_id: u32) -> Drawable {
        if self.pixmaps.contains_key(&drawable_id) {
            Drawable::Pixmap(Pixmap::new(drawable_id))
        } else {
            Drawable::Window(Window::new(drawable_id))
        }
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
            Drawable::Pixmap(p) => {
                let pixmap_id = p.id().get();
                match self.pixmaps.get(&pixmap_id) {
                    Some(&backend_id) => Ok(crate::backend::BackendDrawable::Pixmap(backend_id)),
                    None => Err(format!("Pixmap 0x{:x} not found", pixmap_id).into()),
                }
            }
        }
    }

    // ========== RENDER extension methods ==========

    /// Create a RENDER picture resource
    pub fn create_picture(&mut self, picture_id: u32, drawable: u32, format: u32) {
        log::debug!(
            "Creating picture 0x{:x} for drawable 0x{:x} format {}",
            picture_id,
            drawable,
            format
        );
        self.pictures.insert(
            picture_id,
            Picture {
                drawable,
                format,
                component_alpha: false,
            },
        );
    }

    /// Free a RENDER picture resource
    pub fn free_picture(&mut self, picture_id: u32) {
        log::debug!("Freeing picture 0x{:x}", picture_id);
        self.pictures.remove(&picture_id);
        self.solid_fills.remove(&picture_id);
    }

    /// Get a picture by ID
    pub fn get_picture(&self, picture_id: u32) -> Option<&Picture> {
        self.pictures.get(&picture_id)
    }

    /// Create a RENDER solid fill picture
    pub fn create_solid_fill(
        &mut self,
        picture_id: u32,
        red: u16,
        green: u16,
        blue: u16,
        alpha: u16,
    ) {
        log::debug!(
            "Creating solid fill 0x{:x}: rgba({}, {}, {}, {})",
            picture_id,
            red,
            green,
            blue,
            alpha
        );
        self.solid_fills.insert(
            picture_id,
            SolidFill {
                red,
                green,
                blue,
                alpha,
            },
        );
    }

    /// Get a solid fill by ID
    pub fn get_solid_fill(&self, picture_id: u32) -> Option<&SolidFill> {
        self.solid_fills.get(&picture_id)
    }

    /// Render trapezoids using the RENDER extension
    #[allow(clippy::too_many_arguments)]
    pub fn render_trapezoids(
        &mut self,
        _op: u8,
        src_picture: u32,
        dst_picture: u32,
        _mask_format: u32,
        src_x: i16,
        src_y: i16,
        trapezoids: &[RenderTrapezoid],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Get the source color (from solid fill or picture)
        let (r, g, b, a) = if let Some(solid) = self.get_solid_fill(src_picture) {
            (
                (solid.red >> 8) as u8,
                (solid.green >> 8) as u8,
                (solid.blue >> 8) as u8,
                (solid.alpha >> 8) as u8,
            )
        } else {
            // Default to opaque black if not a solid fill
            (0, 0, 0, 255)
        };

        log::debug!(
            "render_trapezoids: src=0x{:x} dst=0x{:x} color=rgba({},{},{},{}) src_offset=({},{}) {} trapezoids",
            src_picture,
            dst_picture,
            r, g, b, a,
            src_x, src_y,
            trapezoids.len()
        );

        // Get the destination drawable from the picture
        let dst_drawable = if let Some(picture) = self.pictures.get(&dst_picture) {
            picture.drawable
        } else {
            log::warn!("Destination picture 0x{:x} not found", dst_picture);
            return Ok(());
        };

        // Resolve to backend drawable
        let drawable = self.resolve_drawable(dst_drawable);
        let backend_drawable = self.get_backend_drawable(drawable)?;

        // Convert RGBA to pixel value
        let color = ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);

        self.backend
            .fill_trapezoids(backend_drawable, color, trapezoids)?;
        self.backend.flush()?;

        Ok(())
    }

    /// Poll backend events and queue them for delivery to clients
    pub fn poll_and_queue_events(&mut self) {
        use crate::backend::BackendEvent;
        use crate::protocol::Timestamp;

        // Poll events from backend
        let events = match self.backend.poll_events() {
            Ok(events) => events,
            Err(e) => {
                log::warn!("Failed to poll backend events: {:?}", e);
                return;
            }
        };

        for event in events {
            // Map backend window to X11 window
            let x11_window = match &event {
                BackendEvent::KeyPress { window, .. }
                | BackendEvent::KeyRelease { window, .. }
                | BackendEvent::ButtonPress { window, .. }
                | BackendEvent::ButtonRelease { window, .. }
                | BackendEvent::MotionNotify { window, .. }
                | BackendEvent::FocusIn { window }
                | BackendEvent::FocusOut { window }
                | BackendEvent::EnterNotify { window, .. }
                | BackendEvent::LeaveNotify { window, .. }
                | BackendEvent::Expose { window, .. }
                | BackendEvent::Configure { window, .. }
                | BackendEvent::DestroyNotify { window }
                | BackendEvent::MapNotify { window }
                | BackendEvent::UnmapNotify { window } => {
                    if let Some(&x11_win) = self.backend_to_x11.get(window) {
                        x11_win
                    } else {
                        log::debug!("Received event for unknown backend window {:?}", window);
                        continue;
                    }
                }
            };

            // Get window info for event mask checking
            let (event_mask, _width, _height) =
                if let Some(info) = self.window_info.get(&x11_window) {
                    (info.event_mask, info.width, info.height)
                } else {
                    log::debug!(
                        "No window info for X11 window 0x{:x}",
                        x11_window.id().get()
                    );
                    continue;
                };

            // Track pointer position from events that carry coordinates
            match &event {
                BackendEvent::MotionNotify { x, y, .. }
                | BackendEvent::ButtonPress { x, y, .. }
                | BackendEvent::ButtonRelease { x, y, .. }
                | BackendEvent::EnterNotify { x, y, .. } => {
                    self.last_pointer_x = *x;
                    self.last_pointer_y = *y;
                    self.last_pointer_window = x11_window;
                }
                _ => {}
            }

            // Increment sequence number
            self.event_sequence = self.event_sequence.wrapping_add(1);
            let seq = self.event_sequence;

            // Convert backend event to X11 wire format (32 bytes)
            let encoded = match event {
                BackendEvent::KeyPress {
                    keycode,
                    state,
                    time,
                    x,
                    y,
                    ..
                } => {
                    // Find window that wants KeyPress events (bit 0 = 0x0001)
                    // Use event propagation to find child windows that want this event
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x0001) {
                            result
                        } else {
                            log::debug!(
                                "No window wants KeyPress starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    log::debug!(
                        "KeyPress propagated from 0x{:x} to 0x{:x}",
                        x11_window.id().get(),
                        target_window.id().get()
                    );
                    Self::encode_key_event(
                        2, // KeyPress event code
                        keycode,
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        target_window,
                        Window::new(0), // child
                        x,
                        y,
                        x,
                        y,
                        state,
                        true,
                    )
                }
                BackendEvent::KeyRelease {
                    keycode,
                    state,
                    time,
                    x,
                    y,
                    ..
                } => {
                    // Find window that wants KeyRelease events (bit 1 = 0x0002)
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x0002) {
                            result
                        } else {
                            log::debug!(
                                "No window wants KeyRelease starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    Self::encode_key_event(
                        3, // KeyRelease event code
                        keycode,
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        target_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        state,
                        true,
                    )
                }
                BackendEvent::ButtonPress {
                    button,
                    state,
                    time,
                    x,
                    y,
                    ..
                } => {
                    // Find window that wants ButtonPress events (bit 2 = 0x0004)
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x0004) {
                            result
                        } else {
                            log::debug!(
                                "No window wants ButtonPress starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    Self::encode_key_event(
                        4, // ButtonPress event code
                        button,
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        target_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        state,
                        true,
                    )
                }
                BackendEvent::ButtonRelease {
                    button,
                    state,
                    time,
                    x,
                    y,
                    ..
                } => {
                    // Find window that wants ButtonRelease events (bit 3 = 0x0008)
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x0008) {
                            result
                        } else {
                            log::debug!(
                                "No window wants ButtonRelease starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    Self::encode_key_event(
                        5, // ButtonRelease event code
                        button,
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        target_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        state,
                        true,
                    )
                }
                BackendEvent::MotionNotify {
                    state, time, x, y, ..
                } => {
                    // Check if window wants MotionNotify events (bit 6 = PointerMotion)
                    if event_mask & 0x0040 == 0 {
                        continue;
                    }
                    Self::encode_key_event(
                        6, // MotionNotify event code
                        0, // detail (Normal)
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        x11_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        state,
                        true,
                    )
                }
                BackendEvent::FocusIn { .. } => {
                    // Find window that wants FocusChange events (bit 21 = 0x200000)
                    // Use event propagation to find child windows that want this event
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x200000) {
                            result
                        } else {
                            log::debug!(
                                "No window wants FocusIn starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    log::debug!(
                        "FocusIn propagated from 0x{:x} to 0x{:x}",
                        x11_window.id().get(),
                        target_window.id().get()
                    );
                    Self::encode_focus_event(9, 0, seq, target_window, 0) // Normal detail, Normal mode
                }
                BackendEvent::FocusOut { .. } => {
                    // Find window that wants FocusChange events (bit 21 = 0x200000)
                    // Use event propagation to find child windows that want this event
                    let (target_window, _target_mask) =
                        if let Some(result) = self.find_window_for_event(x11_window, 0x200000) {
                            result
                        } else {
                            log::debug!(
                                "No window wants FocusOut starting from 0x{:x}",
                                x11_window.id().get()
                            );
                            continue;
                        };
                    log::debug!(
                        "FocusOut propagated from 0x{:x} to 0x{:x}",
                        x11_window.id().get(),
                        target_window.id().get()
                    );
                    Self::encode_focus_event(10, 0, seq, target_window, 0)
                }
                BackendEvent::EnterNotify { x, y, time, .. } => {
                    // Check if window wants EnterWindow events (bit 4)
                    if event_mask & 0x0010 == 0 {
                        continue;
                    }
                    Self::encode_enter_leave_event(
                        7, // EnterNotify
                        0, // detail (Ancestor)
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        x11_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        0, // state
                        0, // mode (Normal)
                        3, // same_screen_focus (same screen = 1, focus = 2)
                    )
                }
                BackendEvent::LeaveNotify { x, y, time, .. } => {
                    // Check if window wants LeaveWindow events (bit 5)
                    if event_mask & 0x0020 == 0 {
                        continue;
                    }
                    Self::encode_enter_leave_event(
                        8, // LeaveNotify
                        0,
                        seq,
                        Timestamp::new(time),
                        self.root_window,
                        x11_window,
                        Window::new(0),
                        x,
                        y,
                        x,
                        y,
                        0,
                        0,
                        3,
                    )
                }
                BackendEvent::Expose {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    // Check if window wants Exposure events (bit 15)
                    if event_mask & 0x8000 == 0 {
                        continue;
                    }
                    Self::encode_expose_event(seq, x11_window, x, y, width, height, 0)
                }
                BackendEvent::Configure {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => {
                    // Update window info
                    if let Some(info) = self.window_info.get_mut(&x11_window) {
                        info.x = x;
                        info.y = y;
                        info.width = width;
                        info.height = height;
                    }
                    // Check if window wants StructureNotify events (bit 17)
                    if event_mask & 0x20000 == 0 {
                        continue;
                    }
                    Self::encode_configure_notify_event(
                        seq,
                        x11_window,
                        x11_window,
                        Window::new(0),
                        x,
                        y,
                        width,
                        height,
                        0,
                        false,
                    )
                }
                BackendEvent::DestroyNotify { .. }
                | BackendEvent::MapNotify { .. }
                | BackendEvent::UnmapNotify { .. } => {
                    // These are handled elsewhere or not commonly needed
                    continue;
                }
            };

            log::debug!(
                "Queuing event type {} for window 0x{:x}",
                encoded[0],
                x11_window.id().get()
            );

            // Queue the encoded event for the window
            self.pending_events
                .entry(x11_window)
                .or_default()
                .push(encoded);
        }
    }

    /// Take pending events for a specific window
    pub fn take_pending_events(&mut self, window: Window) -> Vec<Vec<u8>> {
        self.pending_events.remove(&window).unwrap_or_default()
    }

    /// Take all pending events (for all windows owned by a client)
    pub fn take_all_pending_events(&mut self) -> HashMap<Window, Vec<Vec<u8>>> {
        std::mem::take(&mut self.pending_events)
    }

    // Helper to encode key/button/motion events (they share the same format)
    #[allow(clippy::too_many_arguments)]
    fn encode_key_event(
        code: u8,
        detail: u8,
        sequence: u16,
        time: Timestamp,
        root: Window,
        event: Window,
        child: Window,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
        same_screen: bool,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = code;
        buffer[1] = detail;
        buffer[2..4].copy_from_slice(&sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&root.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&event.id().get().to_ne_bytes());
        buffer[16..20].copy_from_slice(&child.id().get().to_ne_bytes());
        buffer[20..22].copy_from_slice(&root_x.to_ne_bytes());
        buffer[22..24].copy_from_slice(&root_y.to_ne_bytes());
        buffer[24..26].copy_from_slice(&event_x.to_ne_bytes());
        buffer[26..28].copy_from_slice(&event_y.to_ne_bytes());
        buffer[28..30].copy_from_slice(&state.to_ne_bytes());
        buffer[30] = if same_screen { 1 } else { 0 };
        buffer
    }

    fn encode_focus_event(code: u8, detail: u8, sequence: u16, event: Window, mode: u8) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = code;
        buffer[1] = detail;
        buffer[2..4].copy_from_slice(&sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&event.id().get().to_ne_bytes());
        buffer[8] = mode;
        buffer
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_enter_leave_event(
        code: u8,
        detail: u8,
        sequence: u16,
        time: Timestamp,
        root: Window,
        event: Window,
        child: Window,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
        mode: u8,
        same_screen_focus: u8,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = code;
        buffer[1] = detail;
        buffer[2..4].copy_from_slice(&sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&time.get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&root.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&event.id().get().to_ne_bytes());
        buffer[16..20].copy_from_slice(&child.id().get().to_ne_bytes());
        buffer[20..22].copy_from_slice(&root_x.to_ne_bytes());
        buffer[22..24].copy_from_slice(&root_y.to_ne_bytes());
        buffer[24..26].copy_from_slice(&event_x.to_ne_bytes());
        buffer[26..28].copy_from_slice(&event_y.to_ne_bytes());
        buffer[28..30].copy_from_slice(&state.to_ne_bytes());
        buffer[30] = mode;
        buffer[31] = same_screen_focus;
        buffer
    }

    fn encode_expose_event(
        sequence: u16,
        window: Window,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = 12; // Expose event code
        buffer[2..4].copy_from_slice(&sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&window.id().get().to_ne_bytes());
        buffer[8..10].copy_from_slice(&x.to_ne_bytes());
        buffer[10..12].copy_from_slice(&y.to_ne_bytes());
        buffer[12..14].copy_from_slice(&width.to_ne_bytes());
        buffer[14..16].copy_from_slice(&height.to_ne_bytes());
        buffer[16..18].copy_from_slice(&count.to_ne_bytes());
        buffer
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_configure_notify_event(
        sequence: u16,
        event: Window,
        window: Window,
        above_sibling: Window,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; 32];
        buffer[0] = 22; // ConfigureNotify event code
        buffer[2..4].copy_from_slice(&sequence.to_ne_bytes());
        buffer[4..8].copy_from_slice(&event.id().get().to_ne_bytes());
        buffer[8..12].copy_from_slice(&window.id().get().to_ne_bytes());
        buffer[12..16].copy_from_slice(&above_sibling.id().get().to_ne_bytes());
        buffer[16..18].copy_from_slice(&x.to_ne_bytes());
        buffer[18..20].copy_from_slice(&y.to_ne_bytes());
        buffer[20..22].copy_from_slice(&width.to_ne_bytes());
        buffer[22..24].copy_from_slice(&height.to_ne_bytes());
        buffer[24..26].copy_from_slice(&border_width.to_ne_bytes());
        buffer[26] = if override_redirect { 1 } else { 0 };
        buffer
    }
}
