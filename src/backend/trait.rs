//! Backend trait definition
//!
//! This module defines the trait that all display backends must implement.
//! Backends translate X11 operations to native window system operations.

use crate::protocol::*;
use std::error::Error;

/// Result type for backend operations
pub type BackendResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Backend-specific window handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendWindow(pub usize);

/// Backend-specific drawable handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendDrawable {
    Window(BackendWindow),
    Pixmap(usize),
}

/// Backend-specific graphics context
#[derive(Debug, Clone)]
pub struct BackendGC {
    pub function: GCFunction,
    pub foreground: u32,
    pub background: u32,
    pub line_width: u16,
    pub line_style: LineStyle,
    pub cap_style: CapStyle,
    pub join_style: JoinStyle,
    pub fill_style: FillStyle,
    pub fill_rule: FillRule,
}

impl Default for BackendGC {
    fn default() -> Self {
        BackendGC {
            function: GCFunction::Copy,
            foreground: 0,
            background: 0xffffff,
            line_width: 0,
            line_style: LineStyle::Solid,
            cap_style: CapStyle::Butt,
            join_style: JoinStyle::Miter,
            fill_style: FillStyle::Solid,
            fill_rule: FillRule::EvenOdd,
        }
    }
}

/// Window creation parameters
#[derive(Debug, Clone)]
pub struct WindowParams {
    pub parent: Option<BackendWindow>,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub class: WindowClass,
    pub background_pixel: Option<u32>,
    pub event_mask: u32,
}

/// Window configuration
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub border_width: Option<u16>,
    pub stack_mode: Option<StackMode>,
}

/// Screen information
#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub width: u16,
    pub height: u16,
    pub width_mm: u16,
    pub height_mm: u16,
    pub root_visual: VisualID,
    pub root_depth: u8,
    pub white_pixel: u32,
    pub black_pixel: u32,
}

/// Visual information
#[derive(Debug, Clone)]
pub struct VisualInfo {
    pub visual_id: VisualID,
    pub class: u8,
    pub bits_per_rgb: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}

/// Backend events
#[derive(Debug, Clone)]
pub enum BackendEvent {
    /// Window needs to be redrawn
    Expose {
        window: BackendWindow,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    /// Window was resized by native window manager
    Configure {
        window: BackendWindow,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    },
    /// Key press
    KeyPress {
        window: BackendWindow,
        keycode: u8,
        state: u16,
        time: u32,
        x: i16,
        y: i16,
    },
    /// Key release
    KeyRelease {
        window: BackendWindow,
        keycode: u8,
        state: u16,
        time: u32,
        x: i16,
        y: i16,
    },
    /// Button press
    ButtonPress {
        window: BackendWindow,
        button: u8,
        state: u16,
        time: u32,
        x: i16,
        y: i16,
    },
    /// Button release
    ButtonRelease {
        window: BackendWindow,
        button: u8,
        state: u16,
        time: u32,
        x: i16,
        y: i16,
    },
    /// Mouse motion
    MotionNotify {
        window: BackendWindow,
        state: u16,
        time: u32,
        x: i16,
        y: i16,
    },
    /// Window gained keyboard focus
    FocusIn { window: BackendWindow },
    /// Window lost keyboard focus
    FocusOut { window: BackendWindow },
    /// Window destroyed by user/WM
    DestroyNotify { window: BackendWindow },
}

/// The main backend trait
///
/// All display backends must implement this trait. The trait is designed to be
/// minimal and close to X11 operations, allowing backends to be efficient while
/// still providing the necessary functionality.
pub trait Backend: Send {
    /// Initialize the backend
    fn init(&mut self) -> BackendResult<()>;

    /// Get screen information
    fn get_screen_info(&self) -> BackendResult<ScreenInfo>;

    /// Get available visuals
    fn get_visuals(&self) -> BackendResult<Vec<VisualInfo>>;

    // Window operations

    /// Create a new window
    fn create_window(&mut self, params: WindowParams) -> BackendResult<BackendWindow>;

    /// Destroy a window
    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()>;

    /// Map (show) a window
    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()>;

    /// Unmap (hide) a window
    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()>;

    /// Configure window geometry and stacking
    fn configure_window(
        &mut self,
        window: BackendWindow,
        config: WindowConfig,
    ) -> BackendResult<()>;

    /// Raise window to top
    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()>;

    /// Lower window to bottom
    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()>;

    /// Set window title
    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()>;

    // Drawing operations

    /// Clear an area of a window
    fn clear_area(
        &mut self,
        window: BackendWindow,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()>;

    /// Draw a rectangle outline
    fn draw_rectangle(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()>;

    /// Draw filled rectangle
    fn fill_rectangle(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()>;

    /// Draw multiple rectangles
    fn draw_rectangles(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        rectangles: &[Rectangle],
    ) -> BackendResult<()> {
        // Default implementation: draw one by one
        for rect in rectangles {
            self.draw_rectangle(drawable, gc, rect.x, rect.y, rect.width, rect.height)?;
        }
        Ok(())
    }

    /// Draw multiple filled rectangles
    fn fill_rectangles(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        rectangles: &[Rectangle],
    ) -> BackendResult<()> {
        // Default implementation: draw one by one
        for rect in rectangles {
            self.fill_rectangle(drawable, gc, rect.x, rect.y, rect.width, rect.height)?;
        }
        Ok(())
    }

    /// Draw a line
    fn draw_line(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x1: i16,
        y1: i16,
        x2: i16,
        y2: i16,
    ) -> BackendResult<()>;

    /// Draw multiple connected lines
    fn draw_lines(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()> {
        // Default implementation: draw segments
        for i in 0..points.len().saturating_sub(1) {
            self.draw_line(
                drawable,
                gc,
                points[i].x,
                points[i].y,
                points[i + 1].x,
                points[i + 1].y,
            )?;
        }
        Ok(())
    }

    /// Draw points
    fn draw_points(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()>;

    /// Draw text
    fn draw_text(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        text: &str,
    ) -> BackendResult<()>;

    /// Draw arcs (elliptical arcs)
    fn draw_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[Arc],
    ) -> BackendResult<()>;

    /// Fill arcs (pie slices)
    fn fill_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[Arc],
    ) -> BackendResult<()>;

    /// Fill polygon
    fn fill_polygon(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()>;

    /// Copy area from one drawable to another
    #[allow(clippy::too_many_arguments)]
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
    ) -> BackendResult<()>;

    // Pixmap operations

    /// Create a pixmap (off-screen drawable)
    fn create_pixmap(&mut self, width: u16, height: u16, depth: u8) -> BackendResult<usize>;

    /// Free a pixmap
    fn free_pixmap(&mut self, pixmap: usize) -> BackendResult<()>;

    // Image operations

    /// Put image data to a drawable
    ///
    /// # Arguments
    /// * `drawable` - Target window or pixmap
    /// * `gc` - Graphics context (for clipping, etc.)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `dst_x` - Destination X coordinate
    /// * `dst_y` - Destination Y coordinate
    /// * `depth` - Bits per pixel (1, 8, 16, 24, 32)
    /// * `format` - Image format (0=Bitmap, 1=XYPixmap, 2=ZPixmap)
    /// * `data` - Raw image data
    #[allow(clippy::too_many_arguments)]
    fn put_image(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        data: &[u8],
    ) -> BackendResult<()>;

    /// Get image data from a drawable
    ///
    /// # Arguments
    /// * `drawable` - Source window or pixmap
    /// * `x` - Source X coordinate
    /// * `y` - Source Y coordinate
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `plane_mask` - Plane mask (usually 0xFFFFFFFF for all planes)
    /// * `format` - Desired image format (typically 2=ZPixmap)
    ///
    /// # Returns
    /// Returns image data as Vec<u8> in the requested format
    #[allow(clippy::too_many_arguments)]
    fn get_image(
        &mut self,
        drawable: BackendDrawable,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
        format: u8,
    ) -> BackendResult<Vec<u8>>;

    // Event handling

    /// Poll for events from the backend
    /// This should not block - return empty vec if no events
    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>>;

    /// Flush any pending operations to the display
    fn flush(&mut self) -> BackendResult<()>;

    /// Wait for events (blocking)
    fn wait_for_event(&mut self) -> BackendResult<BackendEvent>;
}
