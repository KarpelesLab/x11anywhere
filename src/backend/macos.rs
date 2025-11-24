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

    fn macos_backend_flush(handle: BackendHandle) -> i32;
}

/// Window data stored per-window
struct WindowData {
    swift_id: i32,
    width: u16,
    height: u16,
    x: i16,
    y: i16,
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
            next_resource_id: 1,
            event_queue: Vec::new(),
            debug: false,
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
                },
            );

            Ok(BackendWindow(id))
        }
    }

    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.remove(&window.0) {
                macos_backend_destroy_window(self.handle, data.swift_id);
            }
            Ok(())
        }
    }

    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                macos_backend_map_window(self.handle, data.swift_id);
            }
            Ok(())
        }
    }

    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                macos_backend_unmap_window(self.handle, data.swift_id);
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

                macos_backend_configure_window(
                    self.handle,
                    data.swift_id,
                    x as i32,
                    y as i32,
                    width as i32,
                    height as i32,
                );

                data.x = x;
                data.y = y;
                data.width = width;
                data.height = height;
            }
            Ok(())
        }
    }

    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                macos_backend_raise_window(self.handle, data.swift_id);
            }
            Ok(())
        }
    }

    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                macos_backend_lower_window(self.handle, data.swift_id);
            }
            Ok(())
        }
    }

    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.get(&window.0) {
                let c_title = CString::new(title).unwrap();
                macos_backend_set_window_title(self.handle, data.swift_id, c_title.as_ptr());
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
            let (is_window, drawable_id) = match drawable {
                BackendDrawable::Window(w) => {
                    if let Some(data) = self.windows.get(&w.0) {
                        (1, data.swift_id)
                    } else {
                        return Err("Window not found".into());
                    }
                }
                BackendDrawable::Pixmap(p) => {
                    if let Some(&swift_id) = self.pixmaps.get(&p) {
                        (0, swift_id)
                    } else {
                        return Err("Pixmap not found".into());
                    }
                }
            };

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
                x as i32,
                y as i32,
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
            let (is_window, drawable_id) = match drawable {
                BackendDrawable::Window(w) => {
                    if let Some(data) = self.windows.get(&w.0) {
                        (1, data.swift_id)
                    } else {
                        return Err("Window not found".into());
                    }
                }
                BackendDrawable::Pixmap(p) => {
                    if let Some(&swift_id) = self.pixmaps.get(&p) {
                        (0, swift_id)
                    } else {
                        return Err("Pixmap not found".into());
                    }
                }
            };

            let (r, g, b) = Self::color_to_rgb(gc.foreground);

            macos_backend_fill_rectangle(
                self.handle,
                is_window,
                drawable_id,
                x as i32,
                y as i32,
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
            let (is_window, drawable_id) = match drawable {
                BackendDrawable::Window(w) => {
                    if let Some(data) = self.windows.get(&w.0) {
                        (1, data.swift_id)
                    } else {
                        return Err("Window not found".into());
                    }
                }
                BackendDrawable::Pixmap(p) => {
                    if let Some(&swift_id) = self.pixmaps.get(&p) {
                        (0, swift_id)
                    } else {
                        return Err("Pixmap not found".into());
                    }
                }
            };

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
                x1 as i32,
                y1 as i32,
                x2 as i32,
                y2 as i32,
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
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _text: &str,
    ) -> BackendResult<()> {
        // TODO: Implement text drawing
        Ok(())
    }

    fn copy_area(
        &mut self,
        _src: BackendDrawable,
        _dst: BackendDrawable,
        _gc: &BackendGC,
        _src_x: i16,
        _src_y: i16,
        _width: u16,
        _height: u16,
        _dst_x: i16,
        _dst_y: i16,
    ) -> BackendResult<()> {
        // TODO: Implement copy_area
        Ok(())
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

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        // TODO: Implement event polling
        Ok(std::mem::take(&mut self.event_queue))
    }

    fn flush(&mut self) -> BackendResult<()> {
        unsafe {
            macos_backend_flush(self.handle);
            Ok(())
        }
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        // TODO: Implement blocking event wait
        // For now, return an error
        Err("wait_for_event not yet implemented".into())
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
