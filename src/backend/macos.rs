//! macOS Backend - Using Swift/Cocoa via FFI
//!
//! This backend provides X11 protocol support on macOS using a Swift wrapper
//! around native Cocoa and Core Graphics APIs. The Swift code handles all
//! Cocoa interactions and exposes a C API that Rust calls via FFI.

use super::*;
use crate::protocol::*;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;

// FFI declarations for Swift backend
type BackendHandle = *mut std::ffi::c_void;

extern "C" {
    fn macos_backend_create() -> BackendHandle;
    fn macos_backend_destroy(handle: BackendHandle);
    fn macos_backend_get_screen_info(
        handle: BackendHandle,
        width: *mut i32,
        height: *mut i32,
        width_mm: *mut i32,
        height_mm: *mut i32,
    ) -> i32;

    fn macos_backend_create_window(
        handle: BackendHandle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> i32;
    fn macos_backend_destroy_window(handle: BackendHandle, window_id: i32) -> i32;
    fn macos_backend_map_window(handle: BackendHandle, window_id: i32) -> i32;
    fn macos_backend_unmap_window(handle: BackendHandle, window_id: i32) -> i32;
    fn macos_backend_configure_window(
        handle: BackendHandle,
        window_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> i32;
    fn macos_backend_raise_window(handle: BackendHandle, window_id: i32) -> i32;
    fn macos_backend_lower_window(handle: BackendHandle, window_id: i32) -> i32;
    fn macos_backend_set_window_title(
        handle: BackendHandle,
        window_id: i32,
        title: *const c_char,
    ) -> i32;

    fn macos_backend_create_pixmap(handle: BackendHandle, width: i32, height: i32) -> i32;
    fn macos_backend_free_pixmap(handle: BackendHandle, pixmap_id: i32) -> i32;

    fn macos_backend_clear_area(
        handle: BackendHandle,
        window_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> i32;
    fn macos_backend_draw_rectangle(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        r: f32,
        g: f32,
        b: f32,
        line_width: f32,
    ) -> i32;
    fn macos_backend_fill_rectangle(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        r: f32,
        g: f32,
        b: f32,
    ) -> i32;
    fn macos_backend_draw_line(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        r: f32,
        g: f32,
        b: f32,
        line_width: f32,
    ) -> i32;

    fn macos_backend_draw_arc(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        angle1: i32,
        angle2: i32,
        r: f32,
        g: f32,
        b: f32,
        line_width: f32,
    ) -> i32;

    fn macos_backend_fill_arc(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        angle1: i32,
        angle2: i32,
        r: f32,
        g: f32,
        b: f32,
    ) -> i32;

    fn macos_backend_fill_polygon(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        points: *const i32,
        point_count: i32,
        r: f32,
        g: f32,
        b: f32,
    ) -> i32;

    fn macos_backend_put_image(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        width: i32,
        height: i32,
        dst_x: i32,
        dst_y: i32,
        depth: i32,
        format: i32,
        data: *const u8,
        data_length: i32,
    ) -> i32;

    fn macos_backend_get_image(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer: *mut u8,
        buffer_size: i32,
    ) -> i32;

    fn macos_backend_copy_area(
        handle: BackendHandle,
        src_is_window: i32,
        src_drawable_id: i32,
        dst_is_window: i32,
        dst_drawable_id: i32,
        src_x: i32,
        src_y: i32,
        width: i32,
        height: i32,
        dst_x: i32,
        dst_y: i32,
    ) -> i32;

    fn macos_backend_flush(handle: BackendHandle) -> i32;

    fn macos_backend_draw_text(
        handle: BackendHandle,
        is_window: i32,
        drawable_id: i32,
        x: i32,
        y: i32,
        text: *const c_char,
        r: f32,
        g: f32,
        b: f32,
    ) -> i32;

    fn macos_backend_poll_event(
        handle: BackendHandle,
        event_type: *mut i32,
        window_id: *mut i32,
        x: *mut i32,
        y: *mut i32,
        width: *mut i32,
        height: *mut i32,
        keycode: *mut i32,
        button: *mut i32,
        state: *mut i32,
        time: *mut i32,
    ) -> i32;
    fn macos_backend_wait_for_event(
        handle: BackendHandle,
        event_type: *mut i32,
        window_id: *mut i32,
        x: *mut i32,
        y: *mut i32,
        width: *mut i32,
        height: *mut i32,
        keycode: *mut i32,
        button: *mut i32,
        state: *mut i32,
        time: *mut i32,
    ) -> i32;

    // Cursor operations
    fn macos_backend_create_cursor(handle: BackendHandle, cursor_type: i32) -> i32;
    fn macos_backend_free_cursor(handle: BackendHandle, cursor_id: i32) -> i32;
    fn macos_backend_set_window_cursor(
        handle: BackendHandle,
        window_id: i32,
        cursor_id: i32,
    ) -> i32;
}

/// Window data stored per-window
struct WindowData {
    /// Swift window ID (for top-level windows) or parent's swift_id (for child windows)
    swift_id: i32,
    width: u16,
    height: u16,
    x: i16,
    y: i16,
    cursor_id: i32,
    /// If this is a child window, stores the parent's backend window ID
    /// Child windows don't create their own NSWindow, they share the parent's
    parent_backend_id: Option<usize>,
}

pub struct MacOSBackend {
    /// Swift backend handle
    handle: BackendHandle,

    /// Whether the backend has been initialized
    initialized: bool,

    /// Screen dimensions
    screen_width: u16,
    screen_height: u16,

    /// Screen physical dimensions in millimeters
    screen_width_mm: u16,
    screen_height_mm: u16,

    /// Window handle mapping (backend window ID -> WindowData)
    windows: HashMap<usize, WindowData>,

    /// Pixmap handle mapping (pixmap ID -> Swift pixmap ID)
    pixmaps: HashMap<usize, i32>,

    /// Cursor handle mapping (cursor ID -> Swift cursor ID)
    cursors: HashMap<usize, i32>,

    /// Next resource ID to allocate
    next_resource_id: usize,

    /// Event queue for polled events
    event_queue: Vec<BackendEvent>,

    /// Debug mode flag
    debug: bool,
}

impl MacOSBackend {
    /// Create a new macOS backend instance
    pub fn new() -> Self {
        Self {
            handle: std::ptr::null_mut(),
            initialized: false,
            screen_width: 1920,
            screen_height: 1080,
            screen_width_mm: 508,
            screen_height_mm: 285,
            windows: HashMap::new(),
            pixmaps: HashMap::new(),
            cursors: HashMap::new(),
            next_resource_id: 1,
            event_queue: Vec::new(),
            debug: false,
        }
    }

    /// Map StandardCursor to macOS cursor type ID
    fn standard_cursor_to_macos_type(cursor: StandardCursor) -> i32 {
        match cursor {
            StandardCursor::LeftPtr | StandardCursor::Arrow | StandardCursor::TopLeftArrow => 0, // arrow
            StandardCursor::Xterm => 1, // IBeam
            StandardCursor::Crosshair | StandardCursor::Cross | StandardCursor::Tcross => 2, // crosshair
            StandardCursor::Hand1 | StandardCursor::Hand2 => 3, // pointingHand
            StandardCursor::Fleur => 4,                         // closedHand (move)
            StandardCursor::SbHDoubleArrow | StandardCursor::DoubleArrow => 5, // resizeLeftRight
            StandardCursor::SbVDoubleArrow => 6,                // resizeUpDown
            StandardCursor::Watch | StandardCursor::Clock => 7, // operationNotAllowed (closest to busy)
            StandardCursor::XCursor | StandardCursor::Pirate => 8, // operationNotAllowed
            _ => 0,                                             // Default to arrow
        }
    }

    /// Enable or disable debug mode
    #[allow(dead_code)]
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Convert X11 color (0xRRGGBB) to RGB components
    fn color_to_rgb(color: u32) -> (f32, f32, f32) {
        let r = ((color >> 16) & 0xff) as f32 / 255.0;
        let g = ((color >> 8) & 0xff) as f32 / 255.0;
        let b = (color & 0xff) as f32 / 255.0;
        (r, g, b)
    }

    /// Get drawable ID from BackendDrawable
    /// Returns (is_window, swift_id, x_offset, y_offset)
    /// For child windows, returns parent's swift_id and child's position as offset
    fn get_drawable_id(&self, drawable: BackendDrawable) -> BackendResult<(i32, i32, i16, i16)> {
        match drawable {
            BackendDrawable::Window(w) => {
                if let Some(data) = self.windows.get(&w.0) {
                    // For child windows, return their position as offset
                    let (x_offset, y_offset) = if data.parent_backend_id.is_some() {
                        (data.x, data.y)
                    } else {
                        (0, 0)
                    };
                    Ok((1, data.swift_id, x_offset, y_offset))
                } else {
                    Err("Window not found".into())
                }
            }
            BackendDrawable::Pixmap(p) => {
                if let Some(&swift_id) = self.pixmaps.get(&p) {
                    Ok((0, swift_id, 0, 0))
                } else {
                    Err("Pixmap not found".into())
                }
            }
        }
    }
}

// Implement Send for MacOSBackend since Swift handles thread safety
unsafe impl Send for MacOSBackend {}

impl Backend for MacOSBackend {
    fn init(&mut self) -> BackendResult<()> {
        unsafe {
            // Create Swift backend handle
            self.handle = macos_backend_create();
            if self.handle.is_null() {
                return Err("Failed to create macOS backend".into());
            }

            // Get screen info from Swift
            let mut width: i32 = 0;
            let mut height: i32 = 0;
            let mut width_mm: i32 = 0;
            let mut height_mm: i32 = 0;

            let result = macos_backend_get_screen_info(
                self.handle,
                &mut width,
                &mut height,
                &mut width_mm,
                &mut height_mm,
            );

            if result != 0 {
                return Err("Failed to get screen info".into());
            }

            self.screen_width = width as u16;
            self.screen_height = height as u16;
            self.screen_width_mm = width_mm as u16;
            self.screen_height_mm = height_mm as u16;

            self.initialized = true;
            Ok(())
        }
    }

    fn get_screen_info(&self) -> BackendResult<ScreenInfo> {
        if !self.initialized {
            return Err("Backend not initialized".into());
        }

        Ok(ScreenInfo {
            width: self.screen_width,
            height: self.screen_height,
            width_mm: self.screen_width_mm,
            height_mm: self.screen_height_mm,
            root_visual: VisualID::new(0x21),
            root_depth: 24,
            white_pixel: 0xffffff,
            black_pixel: 0x000000,
        })
    }

    fn get_visuals(&self) -> BackendResult<Vec<VisualInfo>> {
        if !self.initialized {
            return Err("Backend not initialized".into());
        }

        Ok(vec![VisualInfo {
            visual_id: VisualID::new(0x21),
            class: 4, // TrueColor
            bits_per_rgb: 8,
            colormap_entries: 256,
            red_mask: 0xff0000,
            green_mask: 0x00ff00,
            blue_mask: 0x0000ff,
        }])
    }

    fn create_window(&mut self, params: WindowParams) -> BackendResult<BackendWindow> {
        // Check if this is a child window (has a parent that's not root)
        // params.parent is None for top-level windows (parent=root)
        // params.parent is Some for child windows
        if let Some(parent_backend) = params.parent {
            // This is a child window - don't create a new NSWindow
            // Instead, share the parent's NSWindow and track the relationship
            let parent_swift_id = if let Some(parent_data) = self.windows.get(&parent_backend.0) {
                parent_data.swift_id
            } else {
                return Err("Parent window not found".into());
            };

            let id = self.next_resource_id;
            self.next_resource_id += 1;

            self.windows.insert(
                id,
                WindowData {
                    swift_id: parent_swift_id, // Use parent's swift_id
                    width: params.width,
                    height: params.height,
                    x: params.x,
                    y: params.y,
                    cursor_id: 0,
                    parent_backend_id: Some(parent_backend.0),
                },
            );

            log::debug!(
                "Created child window {} (parent: {}, offset: ({}, {}))",
                id,
                parent_backend.0,
                params.x,
                params.y
            );

            return Ok(BackendWindow(id));
        }

        // Top-level window - create a new NSWindow
        unsafe {
            let swift_id = macos_backend_create_window(
                self.handle,
                params.x as i32,
                params.y as i32,
                params.width as i32,
                params.height as i32,
            );

            if swift_id <= 0 {
                return Err("Failed to create window".into());
            }

            let id = self.next_resource_id;
            self.next_resource_id += 1;

            self.windows.insert(
                id,
                WindowData {
                    swift_id,
                    width: params.width,
                    height: params.height,
                    x: params.x,
                    y: params.y,
                    cursor_id: 0,
                    parent_backend_id: None, // Top-level window
                },
            );

            Ok(BackendWindow(id))
        }
    }

    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.remove(&window.0) {
                // Only destroy the NSWindow for top-level windows
                if data.parent_backend_id.is_none() {
                    macos_backend_destroy_window(self.handle, data.swift_id);
                }
            }
            Ok(())
        }
    }

    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                // Only map top-level windows (child windows are rendered via parent)
                if data.parent_backend_id.is_none() {
                    macos_backend_map_window(self.handle, data.swift_id);
                }
                // Generate MapNotify event for all windows
                self.event_queue.push(BackendEvent::MapNotify { window });
            }
            Ok(())
        }
    }

    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                // Only unmap top-level windows
                if data.parent_backend_id.is_none() {
                    macos_backend_unmap_window(self.handle, data.swift_id);
                }
                // Generate UnmapNotify event for all windows
                self.event_queue.push(BackendEvent::UnmapNotify { window });
            }
            Ok(())
        }
    }

    fn configure_window(
        &mut self,
        window: BackendWindow,
        config: WindowConfig,
    ) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get_mut(&window.0) {
                let x = config.x.unwrap_or(data.x);
                let y = config.y.unwrap_or(data.y);
                let width = config.width.unwrap_or(data.width);
                let height = config.height.unwrap_or(data.height);

                // Check if size changed - we need to generate events
                let size_changed = width != data.width || height != data.height;

                // Only configure the NSWindow for top-level windows
                if data.parent_backend_id.is_none() {
                    macos_backend_configure_window(
                        self.handle,
                        data.swift_id,
                        x as i32,
                        y as i32,
                        width as i32,
                        height as i32,
                    );
                }

                data.x = x;
                data.y = y;
                data.width = width;
                data.height = height;

                // Generate Configure event to notify client of new geometry
                self.event_queue.push(BackendEvent::Configure {
                    window,
                    x,
                    y,
                    width,
                    height,
                });

                // Generate Expose event for the entire window to trigger redraw
                // This is especially important when size changed and buffer was recreated
                if size_changed {
                    self.event_queue.push(BackendEvent::Expose {
                        window,
                        x: 0,
                        y: 0,
                        width,
                        height,
                    });
                }
            }
            Ok(())
        }
    }

    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                // Only raise top-level windows
                if data.parent_backend_id.is_none() {
                    macos_backend_raise_window(self.handle, data.swift_id);
                }
            }
            Ok(())
        }
    }

    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                // Only lower top-level windows
                if data.parent_backend_id.is_none() {
                    macos_backend_lower_window(self.handle, data.swift_id);
                }
            }
            Ok(())
        }
    }

    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                // Only set title for top-level windows
                if data.parent_backend_id.is_none() {
                    let c_title = CString::new(title).unwrap();
                    macos_backend_set_window_title(self.handle, data.swift_id, c_title.as_ptr());
                }
            }
            Ok(())
        }
    }

    fn clear_area(
        &mut self,
        window: BackendWindow,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                macos_backend_clear_area(
                    self.handle,
                    data.swift_id,
                    x as i32,
                    y as i32,
                    width as i32,
                    height as i32,
                );
            }
            Ok(())
        }
    }

    fn draw_rectangle(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;

            let (r, g, b) = Self::color_to_rgb(gc.foreground);
            let line_width = if gc.line_width == 0 {
                1.0
            } else {
                gc.line_width as f32
            };

            macos_backend_draw_rectangle(
                self.handle,
                is_window,
                drawable_id,
                (x + x_offset) as i32,
                (y + y_offset) as i32,
                width as i32,
                height as i32,
                r,
                g,
                b,
                line_width,
            );

            Ok(())
        }
    }

    fn fill_rectangle(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;

            let (r, g, b) = Self::color_to_rgb(gc.foreground);

            macos_backend_fill_rectangle(
                self.handle,
                is_window,
                drawable_id,
                (x + x_offset) as i32,
                (y + y_offset) as i32,
                width as i32,
                height as i32,
                r,
                g,
                b,
            );

            Ok(())
        }
    }

    fn draw_line(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x1: i16,
        y1: i16,
        x2: i16,
        y2: i16,
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;

            let (r, g, b) = Self::color_to_rgb(gc.foreground);
            let line_width = if gc.line_width == 0 {
                1.0
            } else {
                gc.line_width as f32
            };

            macos_backend_draw_line(
                self.handle,
                is_window,
                drawable_id,
                (x1 + x_offset) as i32,
                (y1 + y_offset) as i32,
                (x2 + x_offset) as i32,
                (y2 + y_offset) as i32,
                r,
                g,
                b,
                line_width,
            );

            Ok(())
        }
    }

    fn draw_points(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()> {
        // Draw points as 1x1 rectangles
        for point in points {
            self.fill_rectangle(drawable, gc, point.x, point.y, 1, 1)?;
        }
        Ok(())
    }

    fn draw_text(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        text: &str,
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;
            let (r, g, b) = Self::color_to_rgb(gc.foreground);
            let text_cstr = CString::new(text).unwrap_or_else(|_| CString::new("").unwrap());

            macos_backend_draw_text(
                self.handle,
                is_window,
                drawable_id,
                (x + x_offset) as i32,
                (y + y_offset) as i32,
                text_cstr.as_ptr(),
                r,
                g,
                b,
            );

            Ok(())
        }
    }

    fn draw_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[crate::protocol::Arc],
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;
            let (r, g, b) = Self::color_to_rgb(gc.foreground);
            let line_width = if gc.line_width == 0 {
                1.0
            } else {
                gc.line_width as f32
            };

            for arc in arcs {
                macos_backend_draw_arc(
                    self.handle,
                    is_window,
                    drawable_id,
                    (arc.x + x_offset) as i32,
                    (arc.y + y_offset) as i32,
                    arc.width as i32,
                    arc.height as i32,
                    arc.angle1 as i32,
                    arc.angle2 as i32,
                    r,
                    g,
                    b,
                    line_width,
                );
            }

            Ok(())
        }
    }

    fn fill_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[crate::protocol::Arc],
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;
            let (r, g, b) = Self::color_to_rgb(gc.foreground);

            for arc in arcs {
                macos_backend_fill_arc(
                    self.handle,
                    is_window,
                    drawable_id,
                    (arc.x + x_offset) as i32,
                    (arc.y + y_offset) as i32,
                    arc.width as i32,
                    arc.height as i32,
                    arc.angle1 as i32,
                    arc.angle2 as i32,
                    r,
                    g,
                    b,
                );
            }

            Ok(())
        }
    }

    fn fill_polygon(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[crate::protocol::Point],
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;
            let (r, g, b) = Self::color_to_rgb(gc.foreground);

            // Convert points to flat array of i32, applying child window offset
            let coords: Vec<i32> = points
                .iter()
                .flat_map(|p| vec![(p.x + x_offset) as i32, (p.y + y_offset) as i32])
                .collect();

            macos_backend_fill_polygon(
                self.handle,
                is_window,
                drawable_id,
                coords.as_ptr(),
                points.len() as i32,
                r,
                g,
                b,
            );

            Ok(())
        }
    }

    fn copy_area(
        &mut self,
        src: BackendDrawable,
        dst: BackendDrawable,
        _gc: &BackendGC,
        src_x: i16,
        src_y: i16,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
    ) -> BackendResult<()> {
        unsafe {
            let (src_is_window, src_drawable_id, src_x_offset, src_y_offset) =
                self.get_drawable_id(src)?;
            let (dst_is_window, dst_drawable_id, dst_x_offset, dst_y_offset) =
                self.get_drawable_id(dst)?;

            let result = macos_backend_copy_area(
                self.handle,
                src_is_window,
                src_drawable_id,
                dst_is_window,
                dst_drawable_id,
                (src_x + src_x_offset) as i32,
                (src_y + src_y_offset) as i32,
                width as i32,
                height as i32,
                (dst_x + dst_x_offset) as i32,
                (dst_y + dst_y_offset) as i32,
            );

            if result != 0 {
                Err("Failed to copy area".into())
            } else {
                Ok(())
            }
        }
    }

    fn create_pixmap(&mut self, width: u16, height: u16, _depth: u8) -> BackendResult<usize> {
        unsafe {
            let swift_id = macos_backend_create_pixmap(self.handle, width as i32, height as i32);

            if swift_id <= 0 {
                return Err("Failed to create pixmap".into());
            }

            let id = self.next_resource_id;
            self.next_resource_id += 1;
            self.pixmaps.insert(id, swift_id);

            Ok(id)
        }
    }

    fn free_pixmap(&mut self, pixmap: usize) -> BackendResult<()> {
        unsafe {
            if let Some(swift_id) = self.pixmaps.remove(&pixmap) {
                macos_backend_free_pixmap(self.handle, swift_id);
            }
            Ok(())
        }
    }

    fn put_image(
        &mut self,
        drawable: BackendDrawable,
        _gc: &BackendGC,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        data: &[u8],
    ) -> BackendResult<()> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;

            let result = macos_backend_put_image(
                self.handle,
                is_window,
                drawable_id,
                width as i32,
                height as i32,
                (dst_x + x_offset) as i32,
                (dst_y + y_offset) as i32,
                depth as i32,
                format as i32,
                data.as_ptr(),
                data.len() as i32,
            );

            if result != 0 {
                Err("Failed to put image".into())
            } else {
                Ok(())
            }
        }
    }

    fn get_image(
        &mut self,
        drawable: BackendDrawable,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        _plane_mask: u32,
        _format: u8,
    ) -> BackendResult<(u8, u32, Vec<u8>)> {
        unsafe {
            let (is_window, drawable_id, x_offset, y_offset) = self.get_drawable_id(drawable)?;
            let depth = 24u8;
            let visual = 0x21u32; // TrueColor visual

            // Allocate buffer for 32bpp RGBA data
            let buffer_size = (width as usize) * (height as usize) * 4;
            let mut buffer = vec![0u8; buffer_size];

            let result = macos_backend_get_image(
                self.handle,
                is_window,
                drawable_id,
                (x + x_offset) as i32,
                (y + y_offset) as i32,
                width as i32,
                height as i32,
                buffer.as_mut_ptr(),
                buffer_size as i32,
            );

            if result != 0 {
                Err("Failed to get image".into())
            } else {
                Ok((depth, visual, buffer))
            }
        }
    }

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        unsafe {
            let mut event_type = 0i32;
            let mut window_id = 0i32;
            let mut x = 0i32;
            let mut y = 0i32;
            let mut width = 0i32;
            let mut height = 0i32;
            let mut keycode = 0i32;
            let mut button = 0i32;
            let mut state = 0i32;
            let mut time = 0i32;

            let has_event = macos_backend_poll_event(
                self.handle,
                &mut event_type,
                &mut window_id,
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                &mut keycode,
                &mut button,
                &mut state,
                &mut time,
            );

            if has_event != 0 {
                if let Some(event) = self.convert_event(
                    event_type, window_id, x, y, width, height, keycode, button, state, time,
                ) {
                    self.event_queue.push(event);
                }
            }

            Ok(std::mem::take(&mut self.event_queue))
        }
    }

    fn create_standard_cursor(
        &mut self,
        cursor_shape: StandardCursor,
    ) -> BackendResult<BackendCursor> {
        unsafe {
            let cursor_type = Self::standard_cursor_to_macos_type(cursor_shape);
            let swift_cursor_id = macos_backend_create_cursor(self.handle, cursor_type);
            if swift_cursor_id <= 0 {
                return Err("Failed to create cursor".into());
            }

            let id = self.next_resource_id;
            self.next_resource_id += 1;
            self.cursors.insert(id, swift_cursor_id);
            Ok(BackendCursor(id))
        }
    }

    fn free_cursor(&mut self, cursor: BackendCursor) -> BackendResult<()> {
        if let Some(swift_cursor_id) = self.cursors.remove(&cursor.0) {
            unsafe {
                macos_backend_free_cursor(self.handle, swift_cursor_id);
            }
        }
        Ok(())
    }

    fn set_window_cursor(
        &mut self,
        window: BackendWindow,
        cursor: BackendCursor,
    ) -> BackendResult<()> {
        unsafe {
            let swift_cursor_id = if cursor == BackendCursor::NONE {
                0 // Default cursor
            } else {
                match self.cursors.get(&cursor.0) {
                    Some(&id) => id,
                    None => return Err("Invalid cursor handle".into()),
                }
            };

            if let Some(window_data) = self.windows.get_mut(&window.0) {
                window_data.cursor_id = swift_cursor_id;
                macos_backend_set_window_cursor(self.handle, window_data.swift_id, swift_cursor_id);
            }
            Ok(())
        }
    }

    fn flush(&mut self) -> BackendResult<()> {
        unsafe {
            macos_backend_flush(self.handle);
            Ok(())
        }
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        unsafe {
            let mut event_type = 0i32;
            let mut window_id = 0i32;
            let mut x = 0i32;
            let mut y = 0i32;
            let mut width = 0i32;
            let mut height = 0i32;
            let mut keycode = 0i32;
            let mut button = 0i32;
            let mut state = 0i32;
            let mut time = 0i32;

            let has_event = macos_backend_wait_for_event(
                self.handle,
                &mut event_type,
                &mut window_id,
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                &mut keycode,
                &mut button,
                &mut state,
                &mut time,
            );

            if has_event != 0 {
                if let Some(event) = self.convert_event(
                    event_type, window_id, x, y, width, height, keycode, button, state, time,
                ) {
                    return Ok(event);
                }
            }

            // If no valid event, return an error
            Err("Failed to get event".into())
        }
    }

    fn list_system_fonts(&mut self) -> BackendResult<Vec<BackendFontInfo>> {
        // TODO: Implement font enumeration via CoreText FFI
        // This would require adding Swift FFI functions to enumerate system fonts
        // For now, return some common macOS system fonts as a placeholder
        log::debug!("macOS backend: list_system_fonts called (returning placeholder fonts)");

        Ok(vec![
            BackendFontInfo {
                xlfd_name: "-*-helvetica-medium-r-normal--12-120-75-75-p-0-iso8859-1".to_string(),
                family: "Helvetica".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 0,
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-*-helvetica-bold-r-normal--12-120-75-75-p-0-iso8859-1".to_string(),
                family: "Helvetica".to_string(),
                weight: "bold".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 0,
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-*-times-medium-r-normal--12-120-75-75-p-0-iso8859-1".to_string(),
                family: "Times".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 0,
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-*-courier-medium-r-normal--12-120-75-75-m-70-iso8859-1".to_string(),
                family: "Courier".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 70,
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-*-monaco-medium-r-normal--12-120-75-75-m-70-iso8859-1".to_string(),
                family: "Monaco".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 70,
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
        ])
    }
}

impl MacOSBackend {
    /// Convert Swift event data to BackendEvent
    #[allow(clippy::too_many_arguments)]
    fn convert_event(
        &self,
        event_type: i32,
        window_id: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        keycode: i32,
        button: i32,
        state: i32,
        time: i32,
    ) -> Option<BackendEvent> {
        // Find the BackendWindow for this Swift window ID
        let window = self
            .windows
            .iter()
            .find(|(_, data)| data.swift_id == window_id)
            .map(|(id, _)| BackendWindow(*id))?;

        // Event type mapping from Swift backend:
        // 2 = KeyPress, 3 = KeyRelease, 4 = ButtonPress, 5 = ButtonRelease, 6 = MotionNotify
        // 8 = FocusIn, 9 = FocusOut, 10 = EnterNotify, 11 = LeaveNotify
        match event_type {
            2 => Some(BackendEvent::KeyPress {
                window,
                keycode: keycode as u8,
                state: state as u16,
                time: time as u32,
                x: x as i16,
                y: y as i16,
            }),
            3 => Some(BackendEvent::KeyRelease {
                window,
                keycode: keycode as u8,
                state: state as u16,
                time: time as u32,
                x: x as i16,
                y: y as i16,
            }),
            4 => Some(BackendEvent::ButtonPress {
                window,
                button: button as u8,
                state: state as u16,
                time: time as u32,
                x: x as i16,
                y: y as i16,
            }),
            5 => Some(BackendEvent::ButtonRelease {
                window,
                button: button as u8,
                state: state as u16,
                time: time as u32,
                x: x as i16,
                y: y as i16,
            }),
            6 => Some(BackendEvent::MotionNotify {
                window,
                state: state as u16,
                time: time as u32,
                x: x as i16,
                y: y as i16,
            }),
            8 => Some(BackendEvent::FocusIn { window }),
            9 => Some(BackendEvent::FocusOut { window }),
            10 => Some(BackendEvent::EnterNotify {
                window,
                x: x as i16,
                y: y as i16,
                time: time as u32,
            }),
            11 => Some(BackendEvent::LeaveNotify {
                window,
                x: x as i16,
                y: y as i16,
                time: time as u32,
            }),
            _ => None,
        }
    }
}

impl Drop for MacOSBackend {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                macos_backend_destroy(self.handle);
            }
        }
    }
}
