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

/// Backend-specific cursor handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendCursor(pub usize);

impl BackendCursor {
    /// No cursor (use parent's cursor)
    pub const NONE: BackendCursor = BackendCursor(0);
}

/// Standard X11 cursor shapes (from cursor font glyph indices)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StandardCursor {
    XCursor = 0,
    Arrow = 2,
    BasedArrowDown = 4,
    BasedArrowUp = 6,
    Boat = 8,
    Bogosity = 10,
    BottomLeftCorner = 12,
    BottomRightCorner = 14,
    BottomSide = 16,
    BottomTee = 18,
    BoxSpiral = 20,
    CenterPtr = 22,
    Circle = 24,
    Clock = 26,
    CoffeeMug = 28,
    Cross = 30,
    CrossReverse = 32,
    Crosshair = 34,
    DiamondCross = 36,
    Dot = 38,
    Dotbox = 40,
    DoubleArrow = 42,
    DraftLarge = 44,
    DraftSmall = 46,
    DrapedBox = 48,
    Exchange = 50,
    Fleur = 52,
    Gobbler = 54,
    Gumby = 56,
    Hand1 = 58,
    Hand2 = 60,
    Heart = 62,
    Icon = 64,
    IronCross = 66,
    LeftPtr = 68,
    LeftSide = 70,
    LeftTee = 72,
    Leftbutton = 74,
    LlAngle = 76,
    LrAngle = 78,
    Man = 80,
    Middlebutton = 82,
    Mouse = 84,
    Pencil = 86,
    Pirate = 88,
    Plus = 90,
    QuestionArrow = 92,
    RightPtr = 94,
    RightSide = 96,
    RightTee = 98,
    Rightbutton = 100,
    RtlLogo = 102,
    Sailboat = 104,
    SbDownArrow = 106,
    SbHDoubleArrow = 108,
    SbLeftArrow = 110,
    SbRightArrow = 112,
    SbUpArrow = 114,
    SbVDoubleArrow = 116,
    Shuttle = 118,
    Sizing = 120,
    Spider = 122,
    Spraycan = 124,
    Star = 126,
    Target = 128,
    Tcross = 130,
    TopLeftArrow = 132,
    TopLeftCorner = 134,
    TopRightCorner = 136,
    TopSide = 138,
    TopTee = 140,
    Trek = 142,
    UlAngle = 144,
    Umbrella = 146,
    UrAngle = 148,
    Watch = 150,
    Xterm = 152,
}

impl StandardCursor {
    /// Convert a glyph index from the cursor font to a StandardCursor
    pub fn from_glyph(glyph: u16) -> Option<StandardCursor> {
        match glyph {
            0 => Some(StandardCursor::XCursor),
            2 => Some(StandardCursor::Arrow),
            34 => Some(StandardCursor::Crosshair),
            52 => Some(StandardCursor::Fleur),
            58 => Some(StandardCursor::Hand1),
            60 => Some(StandardCursor::Hand2),
            68 => Some(StandardCursor::LeftPtr),
            150 => Some(StandardCursor::Watch),
            152 => Some(StandardCursor::Xterm),
            // Add more as needed
            _ => None,
        }
    }
}

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

/// A trapezoid for RENDER extension (fixed-point 16.16 format)
#[derive(Debug, Clone, Copy)]
pub struct RenderTrapezoid {
    pub top: i32,
    pub bottom: i32,
    pub left_x1: i32,
    pub left_y1: i32,
    pub left_x2: i32,
    pub left_y2: i32,
    pub right_x1: i32,
    pub right_y1: i32,
    pub right_x2: i32,
    pub right_y2: i32,
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
    /// Window was mapped (shown)
    MapNotify { window: BackendWindow },
    /// Window was unmapped (hidden)
    UnmapNotify { window: BackendWindow },
    /// Pointer entered window
    EnterNotify {
        window: BackendWindow,
        x: i16,
        y: i16,
        time: u32,
    },
    /// Pointer left window
    LeaveNotify {
        window: BackendWindow,
        x: i16,
        y: i16,
        time: u32,
    },
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

    /// Draw line segments (each segment is independent: x1, y1, x2, y2)
    fn draw_segments(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        segments: &[(i16, i16, i16, i16)],
    ) -> BackendResult<()> {
        // Default implementation: draw each segment
        for &(x1, y1, x2, y2) in segments {
            self.draw_line(drawable, gc, x1, y1, x2, y2)?;
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

    /// Fill trapezoids (RENDER extension)
    ///
    /// Trapezoids are defined in fixed-point 16.16 format.
    /// The color is in ARGB format (0xAARRGGBB).
    fn fill_trapezoids(
        &mut self,
        drawable: BackendDrawable,
        color: u32,
        trapezoids: &[RenderTrapezoid],
    ) -> BackendResult<()> {
        // Default implementation: rasterize trapezoids as polygons
        for trap in trapezoids {
            // Convert fixed-point to integer (shift right 16 bits)
            let top = (trap.top >> 16) as i16;
            let bottom = (trap.bottom >> 16) as i16;

            // Calculate left edge X at top and bottom
            let left_x_top = interpolate_x(
                trap.left_x1,
                trap.left_y1,
                trap.left_x2,
                trap.left_y2,
                trap.top,
            );
            let left_x_bottom = interpolate_x(
                trap.left_x1,
                trap.left_y1,
                trap.left_x2,
                trap.left_y2,
                trap.bottom,
            );

            // Calculate right edge X at top and bottom
            let right_x_top = interpolate_x(
                trap.right_x1,
                trap.right_y1,
                trap.right_x2,
                trap.right_y2,
                trap.top,
            );
            let right_x_bottom = interpolate_x(
                trap.right_x1,
                trap.right_y1,
                trap.right_x2,
                trap.right_y2,
                trap.bottom,
            );

            // Convert to integer coordinates
            let x1 = (left_x_top >> 16) as i16;
            let x2 = (right_x_top >> 16) as i16;
            let x3 = (right_x_bottom >> 16) as i16;
            let x4 = (left_x_bottom >> 16) as i16;

            // Create a polygon from the trapezoid
            let points = vec![
                Point { x: x1, y: top },
                Point { x: x2, y: top },
                Point { x: x3, y: bottom },
                Point { x: x4, y: bottom },
            ];

            // Create a temporary GC with the color
            let gc = BackendGC {
                foreground: color,
                ..Default::default()
            };

            self.fill_polygon(drawable, &gc, &points)?;
        }
        Ok(())
    }

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

    // Cursor operations

    /// Create a standard cursor from a glyph index in the cursor font
    ///
    /// Returns a backend cursor handle that can be used with set_window_cursor.
    /// The glyph index corresponds to the X11 cursor font character.
    fn create_standard_cursor(
        &mut self,
        cursor_shape: StandardCursor,
    ) -> BackendResult<BackendCursor> {
        // Default implementation: return NONE (no cursor change)
        let _ = cursor_shape;
        Ok(BackendCursor::NONE)
    }

    /// Free a cursor
    fn free_cursor(&mut self, _cursor: BackendCursor) -> BackendResult<()> {
        // Default implementation: no-op (most backends use system cursors)
        Ok(())
    }

    /// Set the cursor for a window
    ///
    /// If cursor is BackendCursor::NONE, the window uses its parent's cursor.
    fn set_window_cursor(
        &mut self,
        window: BackendWindow,
        cursor: BackendCursor,
    ) -> BackendResult<()> {
        // Default implementation: no-op
        let _ = (window, cursor);
        Ok(())
    }

    // Event handling

    /// Poll for events from the backend
    /// This should not block - return empty vec if no events
    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>>;

    /// Flush any pending operations to the display
    fn flush(&mut self) -> BackendResult<()>;

    /// Wait for events (blocking)
    fn wait_for_event(&mut self) -> BackendResult<BackendEvent>;
}

/// Helper function to interpolate X coordinate along a line at a given Y
/// All values are in fixed-point 16.16 format
pub fn interpolate_x(x1: i32, y1: i32, x2: i32, y2: i32, y: i32) -> i32 {
    if y1 == y2 {
        return x1;
    }
    // Linear interpolation: x = x1 + (x2 - x1) * (y - y1) / (y2 - y1)
    let dy = y2 - y1;
    let dx = x2 - x1;
    let t = y - y1;
    x1 + ((dx as i64 * t as i64) / dy as i64) as i32
}
