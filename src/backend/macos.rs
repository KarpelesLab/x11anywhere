//! macOS Backend - Using Cocoa/Core Graphics
//!
//! This backend provides X11 protocol support on macOS using native
//! Cocoa frameworks and Core Graphics APIs. It translates X11 window
//! operations and drawing commands to their macOS equivalents.

use super::*;
use crate::protocol::*;
use cocoa::appkit::{
    NSApplication, NSApplicationActivationPolicyRegular, NSBackingStoreType, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool, NSPoint, NSRect, NSSize, NSString};
use core_graphics::base::CGFloat;
use core_graphics::color::CGColor;
use core_graphics::context::{CGContext, CGContextRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::mem;
use std::ptr;

/// Window data stored per NSWindow
struct WindowData {
    ns_window: id,
    width: u16,
    height: u16,
    x: i16,
    y: i16,
}

pub struct MacOSBackend {
    /// Whether the backend has been initialized
    initialized: bool,

    /// Autorelease pool for Cocoa memory management
    pool: id,

    /// NSApplication instance
    app: id,

    /// Screen dimensions
    screen_width: u16,
    screen_height: u16,

    /// Screen physical dimensions in millimeters
    screen_width_mm: u16,
    screen_height_mm: u16,

    /// Window handle mapping (backend window ID -> WindowData)
    windows: HashMap<usize, WindowData>,

    /// Pixmap handle mapping (pixmap ID -> CGContextRef)
    pixmaps: HashMap<usize, CGContextRef>,

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
            initialized: false,
            pool: nil,
            app: nil,
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

    /// Convert X11 color (0xRRGGBB) to CGColor
    unsafe fn color_to_cgcolor(&self, color: u32) -> CGColor {
        let r = ((color >> 16) & 0xff) as f64 / 255.0;
        let g = ((color >> 8) & 0xff) as f64 / 255.0;
        let b = (color & 0xff) as f64 / 255.0;
        CGColor::rgb(r, g, b, 1.0)
    }

    /// Set up graphics context for drawing
    unsafe fn setup_gc(&self, context: CGContextRef, gc: &BackendGC) {
        let fg_color = self.color_to_cgcolor(gc.foreground);
        CGContext::set_rgb_fill_color(
            context,
            fg_color.components()[0] as CGFloat,
            fg_color.components()[1] as CGFloat,
            fg_color.components()[2] as CGFloat,
            1.0,
        );
        CGContext::set_rgb_stroke_color(
            context,
            fg_color.components()[0] as CGFloat,
            fg_color.components()[1] as CGFloat,
            fg_color.components()[2] as CGFloat,
            1.0,
        );
        CGContext::set_line_width(context, gc.line_width as CGFloat);
    }

    /// Get CGContextRef for a drawable
    unsafe fn get_context(&mut self, drawable: BackendDrawable) -> BackendResult<CGContextRef> {
        match drawable {
            BackendDrawable::Window(BackendWindow(id)) => {
                if let Some(window_data) = self.windows.get(&id) {
                    // Get the window's graphics context
                    let ns_window = window_data.ns_window;
                    let content_view: id = msg_send![ns_window, contentView];
                    if content_view != nil {
                        let _: () = msg_send![content_view, lockFocus];
                        let ns_context: id = class!(NSGraphicsContext);
                        let current_context: id = msg_send![ns_context, currentContext];
                        if current_context != nil {
                            let cg_context: CGContextRef =
                                msg_send![current_context, CGContext];
                            if !cg_context.is_null() {
                                return Ok(cg_context);
                            }
                        }
                    }
                }
                Err(format!("Window {:?} not found or has no context", drawable).into())
            }
            BackendDrawable::Pixmap(id) => {
                if let Some(&context) = self.pixmaps.get(&id) {
                    Ok(context)
                } else {
                    Err(format!("Pixmap {} not found", id).into())
                }
            }
        }
    }

    /// Release focus on window's content view
    unsafe fn release_window_focus(&self, window: BackendWindow) {
        if let Some(window_data) = self.windows.get(&window.0) {
            let ns_window = window_data.ns_window;
            let content_view: id = msg_send![ns_window, contentView];
            if content_view != nil {
                let _: () = msg_send![content_view, unlockFocus];
            }
        }
    }

    /// Convert X11 y-coordinate to macOS (origin at bottom-left)
    fn flip_y(&self, y: i16, height: u16) -> i16 {
        self.screen_height as i16 - y - height as i16
    }
}

impl Backend for MacOSBackend {
    fn init(&mut self) -> BackendResult<()> {
        unsafe {
            // Create autorelease pool
            self.pool = NSAutoreleasePool::new(nil);

            // Initialize NSApplication
            self.app = NSApplication::sharedApplication(nil);
            self.app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            // Get screen dimensions
            let screen_class = class!(NSScreen);
            let main_screen: id = msg_send![screen_class, mainScreen];
            if main_screen != nil {
                let frame: NSRect = msg_send![main_screen, frame];
                self.screen_width = frame.size.width as u16;
                self.screen_height = frame.size.height as u16;

                // Estimate physical size (assume 72 DPI as default, actual DPI may vary)
                let backing_scale: CGFloat = msg_send![main_screen, backingScaleFactor];
                let dpi = 72.0 * backing_scale as f64;
                self.screen_width_mm = ((self.screen_width as f64 * 25.4) / dpi) as u16;
                self.screen_height_mm = ((self.screen_height as f64 * 25.4) / dpi) as u16;
            }

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
            let id = self.next_resource_id;
            self.next_resource_id += 1;

            // Convert X11 coordinates (origin top-left) to macOS (origin bottom-left)
            let y_flipped = self.flip_y(params.y, params.height);

            let frame = NSRect::new(
                NSPoint::new(params.x as f64, y_flipped as f64),
                NSSize::new(params.width as f64, params.height as f64),
            );

            let style_mask = NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSClosableWindowMask
                | NSWindowStyleMask::NSMiniaturizableWindowMask
                | NSWindowStyleMask::NSResizableWindowMask;

            let ns_window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                style_mask,
                NSBackingStoreType::NSBackingStoreBuffered,
                NO,
            );

            if ns_window == nil {
                return Err("Failed to create NSWindow".into());
            }

            // Set background color if specified
            if let Some(bg_pixel) = params.background_pixel {
                let bg_color = self.color_to_cgcolor(bg_pixel);
                let ns_color_class = class!(NSColor);
                let ns_color: id = msg_send![ns_color_class, colorWithRed:bg_color.components()[0] green:bg_color.components()[1] blue:bg_color.components()[2] alpha:1.0];
                let _: () = msg_send![ns_window, setBackgroundColor: ns_color];
            }

            let window_data = WindowData {
                ns_window,
                width: params.width,
                height: params.height,
                x: params.x,
                y: params.y,
            };

            self.windows.insert(id, window_data);
            Ok(BackendWindow(id))
        }
    }

    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.remove(&window.0) {
                let ns_window = window_data.ns_window;
                let _: () = msg_send![ns_window, close];
            }
            Ok(())
        }
    }

    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
            }
            Ok(())
        }
    }

    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let _: () = msg_send![ns_window, orderOut: nil];
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
            if let Some(window_data) = self.windows.get_mut(&window.0) {
                let ns_window = window_data.ns_window;
                let mut frame: NSRect = msg_send![ns_window, frame];

                if let Some(x) = config.x {
                    frame.origin.x = x as f64;
                    window_data.x = x;
                }
                if let Some(y) = config.y {
                    let y_flipped = self.flip_y(y, window_data.height);
                    frame.origin.y = y_flipped as f64;
                    window_data.y = y;
                }
                if let Some(width) = config.width {
                    frame.size.width = width as f64;
                    window_data.width = width;
                }
                if let Some(height) = config.height {
                    frame.size.height = height as f64;
                    window_data.height = height;
                }

                let _: () = msg_send![ns_window, setFrame:frame display:YES];
            }
            Ok(())
        }
    }

    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let _: () = msg_send![ns_window, orderFront: nil];
            }
            Ok(())
        }
    }

    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let _: () = msg_send![ns_window, orderBack: nil];
            }
            Ok(())
        }
    }

    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()> {
        unsafe {
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let ns_title = NSString::alloc(nil).init_str(title);
                let _: () = msg_send![ns_window, setTitle: ns_title];
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
            if let Some(window_data) = self.windows.get(&window.0) {
                let ns_window = window_data.ns_window;
                let content_view: id = msg_send![ns_window, contentView];
                if content_view != nil {
                    let rect = NSRect::new(
                        NSPoint::new(x as f64, y as f64),
                        NSSize::new(width as f64, height as f64),
                    );
                    let _: () = msg_send![content_view, setNeedsDisplayInRect: rect];
                }
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
            let context = self.get_context(drawable)?;
            self.setup_gc(context, gc);

            let rect =
                CGRect::new(&CGPoint::new(x as f64, y as f64), &CGSize::new(width as f64, height as f64));
            CGContext::stroke_rect(context, rect);

            if let BackendDrawable::Window(window) = drawable {
                self.release_window_focus(window);
            }
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
            let context = self.get_context(drawable)?;
            self.setup_gc(context, gc);

            let rect =
                CGRect::new(&CGPoint::new(x as f64, y as f64), &CGSize::new(width as f64, height as f64));
            CGContext::fill_rect(context, rect);

            if let BackendDrawable::Window(window) = drawable {
                self.release_window_focus(window);
            }
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
            let context = self.get_context(drawable)?;
            self.setup_gc(context, gc);

            CGContext::begin_path(context);
            CGContext::move_to_point(context, x1 as f64, y1 as f64);
            CGContext::add_line_to_point(context, x2 as f64, y2 as f64);
            CGContext::stroke_path(context);

            if let BackendDrawable::Window(window) = drawable {
                self.release_window_focus(window);
            }
            Ok(())
        }
    }

    fn draw_points(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()> {
        unsafe {
            let context = self.get_context(drawable)?;
            self.setup_gc(context, gc);

            for point in points {
                let rect = CGRect::new(
                    &CGPoint::new(point.x as f64, point.y as f64),
                    &CGSize::new(1.0, 1.0),
                );
                CGContext::fill_rect(context, rect);
            }

            if let BackendDrawable::Window(window) = drawable {
                self.release_window_focus(window);
            }
            Ok(())
        }
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
            let context = self.get_context(drawable)?;
            self.setup_gc(context, gc);

            // Set text drawing mode
            CGContext::set_text_drawing_mode(
                context,
                core_graphics::context::CGTextDrawingMode::CGTextFill,
            );

            // Draw text at position
            let ns_string = NSString::alloc(nil).init_str(text);
            let point = NSPoint::new(x as f64, y as f64);
            let _: () = msg_send![ns_string, drawAtPoint:point withAttributes:nil];

            if let BackendDrawable::Window(window) = drawable {
                self.release_window_focus(window);
            }
            Ok(())
        }
    }

    fn copy_area(
        &mut self,
        src: BackendDrawable,
        dst: BackendDrawable,
        gc: &BackendGC,
        src_x: i16,
        src_y: i16,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
    ) -> BackendResult<()> {
        unsafe {
            let _src_context = self.get_context(src)?;
            let dst_context = self.get_context(dst)?;
            self.setup_gc(dst_context, gc);

            // Create source rect
            let src_rect = CGRect::new(
                &CGPoint::new(src_x as f64, src_y as f64),
                &CGSize::new(width as f64, height as f64),
            );

            // For now, just fill the destination area (proper bitmap copy requires CGImage)
            let dst_rect = CGRect::new(
                &CGPoint::new(dst_x as f64, dst_y as f64),
                &CGSize::new(width as f64, height as f64),
            );
            CGContext::fill_rect(dst_context, dst_rect);

            if let BackendDrawable::Window(window) = dst {
                self.release_window_focus(window);
            }
            Ok(())
        }
    }

    fn create_pixmap(&mut self, width: u16, height: u16, depth: u8) -> BackendResult<usize> {
        unsafe {
            let id = self.next_resource_id;
            self.next_resource_id += 1;

            // Create bitmap context for the pixmap
            let color_space = core_graphics::color_space::CGColorSpace::create_device_rgb();
            let bytes_per_row = width as usize * 4;

            let context = core_graphics::context::CGContext::create_bitmap_context(
                ptr::null_mut(),
                width as usize,
                height as usize,
                8,
                bytes_per_row,
                &color_space,
                core_graphics::base::kCGImageAlphaPremultipliedLast,
            );

            self.pixmaps.insert(id, context.as_ptr());
            mem::forget(context); // Prevent automatic release
            Ok(id)
        }
    }

    fn free_pixmap(&mut self, pixmap: usize) -> BackendResult<()> {
        if let Some(context) = self.pixmaps.remove(&pixmap) {
            unsafe {
                // Recreate CGContext from raw pointer to allow proper cleanup
                let _context = core_graphics::context::CGContext::from_existing_context_ptr(context);
                // Drops when going out of scope
            }
        }
        Ok(())
    }

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        unsafe {
            // Process pending events
            let until_date: id = msg_send![class!(NSDate), distantPast];
            loop {
                let event: id = msg_send![self.app, nextEventMatchingMask:0xFFFFFFFF untilDate:until_date inMode:cocoa::appkit::NSDefaultRunLoopMode dequeue:YES];
                if event == nil {
                    break;
                }
                let _: () = msg_send![self.app, sendEvent: event];
            }

            Ok(mem::take(&mut self.event_queue))
        }
    }

    fn flush(&mut self) -> BackendResult<()> {
        unsafe {
            // Flush window server
            let _: () = msg_send![self.app, updateWindows];
            Ok(())
        }
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        unsafe {
            loop {
                let until_date: id = msg_send![class!(NSDate), distantFuture];
                let event: id = msg_send![self.app, nextEventMatchingMask:0xFFFFFFFF untilDate:until_date inMode:cocoa::appkit::NSDefaultRunLoopMode dequeue:YES];
                if event != nil {
                    let _: () = msg_send![self.app, sendEvent: event];

                    if !self.event_queue.is_empty() {
                        return Ok(self.event_queue.remove(0));
                    }
                }
            }
        }
    }
}

impl Drop for MacOSBackend {
    fn drop(&mut self) {
        unsafe {
            // Clean up windows
            for (_, window_data) in self.windows.drain() {
                let _: () = msg_send![window_data.ns_window, close];
            }

            // Clean up pixmaps
            for (_, context) in self.pixmaps.drain() {
                let _context = core_graphics::context::CGContext::from_existing_context_ptr(context);
            }

            // Drain autorelease pool
            if self.pool != nil {
                let _: () = msg_send![self.pool, drain];
            }
        }
    }
}
