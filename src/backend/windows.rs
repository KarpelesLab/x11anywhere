//! Windows Backend - Using Win32 APIs
//!
//! This backend provides X11 protocol support on Windows using native
//! Win32 APIs and GDI for drawing. It translates X11 window operations
//! and drawing commands to their Windows equivalents.

use super::*;
use crate::protocol::*;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter::once;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WINDOW_CLASS_NAME: &str = "X11AnywhereWindow";

/// Convert Rust string to wide string for Windows APIs
fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

/// RGB color helper
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Extract RGB components from u32 color (0xRRGGBB)
fn color_to_rgb(color: u32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xff) as u8;
    let g = ((color >> 8) & 0xff) as u8;
    let b = (color & 0xff) as u8;
    (r, g, b)
}

/// Window data stored per-window
struct WindowData {
    hwnd: HWND,
    hdc: HDC,
    /// Backing store memory DC (all drawing goes here)
    mem_dc: HDC,
    /// Backing store bitmap
    mem_bitmap: HBITMAP,
    /// Previous bitmap to restore on cleanup
    old_bitmap: isize,
    width: u16,
    height: u16,
    /// Whether the mouse is currently inside this window
    mouse_inside: bool,
    /// Current cursor for this window
    cursor: HCURSOR,
}

pub struct WindowsBackend {
    /// Whether the backend has been initialized
    initialized: bool,

    /// Module handle for window class
    hinstance: HINSTANCE,

    /// Screen dimensions
    screen_width: u16,
    screen_height: u16,

    /// Screen physical dimensions in millimeters
    screen_width_mm: u16,
    screen_height_mm: u16,

    /// Window handle mapping (Backend window ID -> WindowData)
    windows: HashMap<usize, WindowData>,

    /// Pixmap handle mapping (X11 pixmap ID -> (HDC, HBITMAP))
    pixmaps: HashMap<usize, (HDC, HBITMAP)>,

    /// Cursor handle mapping (Backend cursor ID -> HCURSOR)
    cursors: HashMap<usize, HCURSOR>,

    /// Next resource ID to allocate
    next_resource_id: usize,

    /// Event queue
    event_queue: Vec<BackendEvent>,

    /// Debug mode flag
    debug: bool,
}

impl WindowsBackend {
    /// Create a new Windows backend instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            hinstance: 0,
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

    /// Map StandardCursor to Windows system cursor ID
    fn standard_cursor_to_idc(cursor: StandardCursor) -> *const u16 {
        match cursor {
            StandardCursor::LeftPtr | StandardCursor::Arrow | StandardCursor::TopLeftArrow => {
                IDC_ARROW
            }
            StandardCursor::Xterm => IDC_IBEAM,
            StandardCursor::Watch | StandardCursor::Clock => IDC_WAIT,
            StandardCursor::Crosshair | StandardCursor::Cross | StandardCursor::Tcross => IDC_CROSS,
            StandardCursor::Fleur => IDC_SIZEALL,
            StandardCursor::Hand1 | StandardCursor::Hand2 => IDC_HAND,
            StandardCursor::SbHDoubleArrow | StandardCursor::DoubleArrow => IDC_SIZEWE,
            StandardCursor::SbVDoubleArrow => IDC_SIZENS,
            StandardCursor::TopLeftCorner | StandardCursor::BottomRightCorner => IDC_SIZENWSE,
            StandardCursor::TopRightCorner | StandardCursor::BottomLeftCorner => IDC_SIZENESW,
            StandardCursor::TopSide | StandardCursor::BottomSide => IDC_SIZENS,
            StandardCursor::LeftSide | StandardCursor::RightSide => IDC_SIZEWE,
            StandardCursor::QuestionArrow => IDC_HELP,
            StandardCursor::XCursor | StandardCursor::Pirate => IDC_NO,
            _ => IDC_ARROW, // Default to arrow for unmapped cursors
        }
    }

    /// Enable or disable debug mode
    #[allow(dead_code)]
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Window procedure for handling messages
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CLOSE => {
                // Don't destroy - let X11 client decide
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = mem::zeroed();
                let paint_dc = BeginPaint(hwnd, &mut ps);
                // Blit from backing store (mem_dc stored in GWLP_USERDATA)
                let mem_dc = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as HDC;
                if mem_dc != 0 {
                    let rc = ps.rcPaint;
                    BitBlt(
                        paint_dc,
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        mem_dc,
                        rc.left,
                        rc.top,
                        SRCCOPY,
                    );
                }
                EndPaint(hwnd, &ps);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Register window class
    unsafe fn register_window_class(&self) -> Result<(), String> {
        let class_name = to_wide_string(WINDOW_CLASS_NAME);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_OWNDC,
            lpfnWndProc: Some(Self::wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: self.hinstance,
            hIcon: LoadIconW(0, IDI_APPLICATION),
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: GetStockObject(WHITE_BRUSH) as HBRUSH,
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        if RegisterClassW(&wc) == 0 {
            return Err(format!("Failed to register window class"));
        }

        Ok(())
    }

    /// Get actual HWND for a backend window
    fn get_hwnd(&self, window: BackendWindow) -> Result<HWND, String> {
        self.windows
            .get(&window.0)
            .map(|w| w.hwnd)
            .ok_or_else(|| format!("Invalid window ID: {}", window.0))
    }

    /// Get window data
    fn get_window_data(&self, window: BackendWindow) -> Result<&WindowData, String> {
        self.windows
            .get(&window.0)
            .ok_or_else(|| format!("Invalid window ID: {}", window.0))
    }

    /// Get device context for drawable (returns backing store DC for windows)
    fn get_dc(&self, drawable: BackendDrawable) -> Result<HDC, String> {
        match drawable {
            BackendDrawable::Window(win) => {
                let data = self.get_window_data(win)?;
                Ok(data.mem_dc)
            }
            BackendDrawable::Pixmap(id) => self
                .pixmaps
                .get(&id)
                .map(|(hdc, _)| *hdc)
                .ok_or_else(|| format!("Invalid pixmap ID: {}", id)),
        }
    }

    /// Create GDI pen from GC
    unsafe fn create_pen(&self, gc: &BackendGC) -> HPEN {
        let (r, g, b) = color_to_rgb(gc.foreground);
        let style = match gc.line_style {
            LineStyle::Solid => PS_SOLID,
            LineStyle::OnOffDash => PS_DASH,
            LineStyle::DoubleDash => PS_DASH,
        };
        CreatePen(style as i32, gc.line_width as i32, rgb(r, g, b))
    }

    /// Create GDI brush from GC
    unsafe fn create_brush(&self, gc: &BackendGC) -> HBRUSH {
        let (r, g, b) = color_to_rgb(gc.foreground);
        CreateSolidBrush(rgb(r, g, b))
    }
}

impl Backend for WindowsBackend {
    fn init(&mut self) -> BackendResult<()> {
        unsafe {
            // Get module handle
            self.hinstance = GetModuleHandleW(ptr::null());
            if self.hinstance == 0 {
                return Err("Failed to get module handle".into());
            }

            // Get actual screen dimensions
            self.screen_width = GetSystemMetrics(SM_CXSCREEN) as u16;
            self.screen_height = GetSystemMetrics(SM_CYSCREEN) as u16;

            // Get physical dimensions in mm (approximate using DPI)
            let hdc = GetDC(0);
            if hdc != 0 {
                let dpi_x = GetDeviceCaps(hdc, LOGPIXELSX as i32);
                let dpi_y = GetDeviceCaps(hdc, LOGPIXELSY as i32);
                // Convert pixels to mm (25.4mm per inch)
                self.screen_width_mm = ((self.screen_width as f32 * 25.4) / dpi_x as f32) as u16;
                self.screen_height_mm = ((self.screen_height as f32 * 25.4) / dpi_y as f32) as u16;
                ReleaseDC(0, hdc);
            }

            // Register window class
            self.register_window_class()?;

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

        // Return a basic TrueColor visual
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
            let class_name = to_wide_string(WINDOW_CLASS_NAME);
            let window_name = to_wide_string("X11 Window");

            // Determine window style
            let style = WS_OVERLAPPEDWINDOW;
            let ex_style = WS_EX_APPWINDOW;

            // Calculate the actual window size needed to achieve the desired client area size
            // X11 window dimensions are for the client area, but Windows CreateWindowExW
            // uses the total window size including title bar and borders
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: params.width as i32,
                bottom: params.height as i32,
            };
            AdjustWindowRectEx(&mut rect, style, 0, ex_style);
            let window_width = rect.right - rect.left;
            let window_height = rect.bottom - rect.top;

            // Create the window
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                window_name.as_ptr(),
                style,
                params.x as i32,
                params.y as i32,
                window_width,
                window_height,
                0,
                0,
                self.hinstance,
                ptr::null(),
            );

            if hwnd == 0 {
                return Err("Failed to create window".into());
            }

            // Get device context for drawing
            let hdc = GetDC(hwnd);
            if hdc == 0 {
                DestroyWindow(hwnd);
                return Err("Failed to get device context".into());
            }

            // Create backing store (off-screen memory DC + bitmap)
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bitmap = CreateCompatibleBitmap(hdc, params.width as i32, params.height as i32);
            if mem_dc == 0 || mem_bitmap == 0 {
                if mem_dc != 0 {
                    DeleteDC(mem_dc);
                }
                if mem_bitmap != 0 {
                    DeleteObject(mem_bitmap as isize);
                }
                ReleaseDC(hwnd, hdc);
                DestroyWindow(hwnd);
                return Err("Failed to create backing store".into());
            }
            let old_bitmap = SelectObject(mem_dc, mem_bitmap as isize);

            // Fill backing store with white background
            let bg_rect = RECT {
                left: 0,
                top: 0,
                right: params.width as i32,
                bottom: params.height as i32,
            };
            FillRect(mem_dc, &bg_rect, GetStockObject(WHITE_BRUSH) as HBRUSH);

            // Store mem_dc in GWLP_USERDATA so wnd_proc can access it for WM_PAINT
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, mem_dc as isize);

            let id = self.next_resource_id;
            self.next_resource_id += 1;

            // Load default arrow cursor
            let default_cursor = LoadCursorW(0, IDC_ARROW);

            self.windows.insert(
                id,
                WindowData {
                    hwnd,
                    hdc,
                    mem_dc,
                    mem_bitmap,
                    old_bitmap,
                    width: params.width,
                    height: params.height,
                    mouse_inside: false,
                    cursor: default_cursor,
                },
            );

            Ok(BackendWindow(id))
        }
    }

    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.remove(&window.0) {
                // Clean up backing store
                SelectObject(data.mem_dc, data.old_bitmap);
                DeleteObject(data.mem_bitmap as isize);
                DeleteDC(data.mem_dc);
                // Clear GWLP_USERDATA before destroying
                SetWindowLongPtrW(data.hwnd, GWLP_USERDATA, 0);
                ReleaseDC(data.hwnd, data.hdc);
                DestroyWindow(data.hwnd);
            }
            Ok(())
        }
    }

    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            // Generate MapNotify event
            self.event_queue.push(BackendEvent::MapNotify { window });
            Ok(())
        }
    }

    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            ShowWindow(hwnd, SW_HIDE);
            // Generate UnmapNotify event
            self.event_queue.push(BackendEvent::UnmapNotify { window });
            Ok(())
        }
    }

    fn configure_window(
        &mut self,
        window: BackendWindow,
        config: WindowConfig,
    ) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;

            // Get current window rect if we need to preserve some values
            let mut window_rect: RECT = mem::zeroed();
            GetWindowRect(hwnd, &mut window_rect);

            // Get current client rect to calculate current client area size
            let mut client_rect: RECT = mem::zeroed();
            GetClientRect(hwnd, &mut client_rect);
            let current_client_width = (client_rect.right - client_rect.left) as u16;
            let current_client_height = (client_rect.bottom - client_rect.top) as u16;

            let x = config.x.unwrap_or(window_rect.left as i16) as i32;
            let y = config.y.unwrap_or(window_rect.top as i16) as i32;

            // Calculate new window size accounting for frame
            let new_client_width = config.width.unwrap_or(current_client_width);
            let new_client_height = config.height.unwrap_or(current_client_height);

            // Adjust for window frame to get total window size
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let mut adjust_rect = RECT {
                left: 0,
                top: 0,
                right: new_client_width as i32,
                bottom: new_client_height as i32,
            };
            AdjustWindowRectEx(&mut adjust_rect, style, 0, ex_style);
            let width = adjust_rect.right - adjust_rect.left;
            let height = adjust_rect.bottom - adjust_rect.top;

            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            SetWindowPos(hwnd, 0, x, y, width, height, flags);

            // Update stored dimensions (client area dimensions)
            if let Some(data) = self.windows.get_mut(&window.0) {
                if let Some(w) = config.width {
                    data.width = w;
                }
                if let Some(h) = config.height {
                    data.height = h;
                }
            }

            Ok(())
        }
    }

    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            Ok(())
        }
    }

    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            Ok(())
        }
    }

    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            let title_wide = to_wide_string(title);
            SetWindowTextW(hwnd, title_wide.as_ptr());
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
            let data = self.get_window_data(window)?;
            let hdc = data.mem_dc;

            let rect = RECT {
                left: x as i32,
                top: y as i32,
                right: x as i32 + width as i32,
                bottom: y as i32 + height as i32,
            };

            let brush = GetStockObject(WHITE_BRUSH) as HBRUSH;
            FillRect(hdc, &rect, brush);

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
            let hdc = self.get_dc(drawable)?;

            let pen = self.create_pen(gc);
            let old_pen = SelectObject(hdc, pen as isize);

            // Set brush to null for outline only
            let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH) as isize);

            Rectangle(
                hdc,
                x as i32,
                y as i32,
                x as i32 + width as i32,
                y as i32 + height as i32,
            );

            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(pen as isize);

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
            let hdc = self.get_dc(drawable)?;

            let (r, g, b) = color_to_rgb(gc.foreground);
            let color = rgb(r, g, b);
            log::debug!(
                "Windows fill_rectangle: foreground=0x{:08x}, RGB=({},{},{}), COLORREF=0x{:08x}",
                gc.foreground,
                r,
                g,
                b,
                color
            );

            // Use DC_BRUSH with SetDCBrushColor for more reliable color rendering
            let dc_brush = GetStockObject(DC_BRUSH) as HBRUSH;
            SetDCBrushColor(hdc, color);

            let rect = RECT {
                left: x as i32,
                top: y as i32,
                right: x as i32 + width as i32,
                bottom: y as i32 + height as i32,
            };

            FillRect(hdc, &rect, dc_brush);

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
            let hdc = self.get_dc(drawable)?;

            let pen = self.create_pen(gc);
            let old_pen = SelectObject(hdc, pen as isize);

            MoveToEx(hdc, x1 as i32, y1 as i32, ptr::null_mut());
            LineTo(hdc, x2 as i32, y2 as i32);

            SelectObject(hdc, old_pen);
            DeleteObject(pen as isize);

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
            let hdc = self.get_dc(drawable)?;
            let (r, g, b) = color_to_rgb(gc.foreground);
            let color = rgb(r, g, b);

            for point in points {
                SetPixel(hdc, point.x as i32, point.y as i32, color);
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
            let hdc = self.get_dc(drawable)?;
            let text_wide = to_wide_string(text);

            let (r, g, b) = color_to_rgb(gc.foreground);
            SetTextColor(hdc, rgb(r, g, b));
            SetBkMode(hdc, TRANSPARENT as i32);

            TextOutW(
                hdc,
                x as i32,
                y as i32,
                text_wide.as_ptr(),
                (text_wide.len() - 1) as i32,
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
            let hdc = self.get_dc(drawable)?;
            let pen = self.create_pen(gc);
            let old_pen = SelectObject(hdc, pen);

            for arc in arcs {
                // X11 arcs are defined by a bounding rectangle and start/end angles
                // Angles are in 1/64th of a degree, with 0 at 3 o'clock, counterclockwise
                let left = arc.x as i32;
                let top = arc.y as i32;
                let right = (arc.x + arc.width as i16) as i32;
                let bottom = (arc.y + arc.height as i16) as i32;

                // Convert X11 angles (1/64 degrees) to radians
                let start_angle = (arc.angle1 as f64) * std::f64::consts::PI / (180.0 * 64.0);
                let end_angle =
                    ((arc.angle1 + arc.angle2) as f64) * std::f64::consts::PI / (180.0 * 64.0);

                // Calculate start and end points on the ellipse
                let center_x = (left + right) as f64 / 2.0;
                let center_y = (top + bottom) as f64 / 2.0;
                let radius_x = (right - left) as f64 / 2.0;
                let radius_y = (bottom - top) as f64 / 2.0;

                let start_x = (center_x + radius_x * start_angle.cos()) as i32;
                let start_y = (center_y - radius_y * start_angle.sin()) as i32;
                let end_x = (center_x + radius_x * end_angle.cos()) as i32;
                let end_y = (center_y - radius_y * end_angle.sin()) as i32;

                // Use Arc() to draw the arc
                windows_sys::Win32::Graphics::Gdi::Arc(
                    hdc, left, top, right, bottom, start_x, start_y, end_x, end_y,
                );
            }

            SelectObject(hdc, old_pen);
            DeleteObject(pen);
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
            let hdc = self.get_dc(drawable)?;
            let brush = self.create_brush(gc);
            let old_brush = SelectObject(hdc, brush);

            for arc in arcs {
                let left = arc.x as i32;
                let top = arc.y as i32;
                let right = (arc.x + arc.width as i16) as i32;
                let bottom = (arc.y + arc.height as i16) as i32;

                let start_angle = (arc.angle1 as f64) * std::f64::consts::PI / (180.0 * 64.0);
                let end_angle =
                    ((arc.angle1 + arc.angle2) as f64) * std::f64::consts::PI / (180.0 * 64.0);

                let center_x = (left + right) as f64 / 2.0;
                let center_y = (top + bottom) as f64 / 2.0;
                let radius_x = (right - left) as f64 / 2.0;
                let radius_y = (bottom - top) as f64 / 2.0;

                let start_x = (center_x + radius_x * start_angle.cos()) as i32;
                let start_y = (center_y - radius_y * start_angle.sin()) as i32;
                let end_x = (center_x + radius_x * end_angle.cos()) as i32;
                let end_y = (center_y - radius_y * end_angle.sin()) as i32;

                // Use Pie() to draw filled pie slices
                Pie(
                    hdc, left, top, right, bottom, start_x, start_y, end_x, end_y,
                );
            }

            SelectObject(hdc, old_brush);
            DeleteObject(brush);
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
            let hdc = self.get_dc(drawable)?;
            let brush = self.create_brush(gc);
            let old_brush = SelectObject(hdc, brush);

            // Convert points to Windows POINT structure
            let win_points: Vec<POINT> = points
                .iter()
                .map(|p| POINT {
                    x: p.x as i32,
                    y: p.y as i32,
                })
                .collect();

            // Draw filled polygon
            Polygon(hdc, win_points.as_ptr(), win_points.len() as i32);

            SelectObject(hdc, old_brush);
            DeleteObject(brush);
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
            let src_hdc = self.get_dc(src)?;
            let dst_hdc = self.get_dc(dst)?;

            BitBlt(
                dst_hdc,
                dst_x as i32,
                dst_y as i32,
                width as i32,
                height as i32,
                src_hdc,
                src_x as i32,
                src_y as i32,
                SRCCOPY,
            );

            Ok(())
        }
    }

    fn create_pixmap(&mut self, width: u16, height: u16, _depth: u8) -> BackendResult<usize> {
        unsafe {
            // Create a memory DC compatible with the screen
            let screen_dc = GetDC(0);
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
            ReleaseDC(0, screen_dc);

            if mem_dc == 0 || bitmap == 0 {
                if mem_dc != 0 {
                    DeleteDC(mem_dc);
                }
                return Err("Failed to create pixmap".into());
            }

            // Select bitmap into DC
            SelectObject(mem_dc, bitmap as isize);

            let id = self.next_resource_id;
            self.next_resource_id += 1;
            self.pixmaps.insert(id, (mem_dc, bitmap));

            Ok(id)
        }
    }

    fn free_pixmap(&mut self, pixmap: usize) -> BackendResult<()> {
        unsafe {
            if let Some((hdc, hbitmap)) = self.pixmaps.remove(&pixmap) {
                DeleteObject(hbitmap as isize);
                DeleteDC(hdc);
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
            let hdc = self.get_dc(drawable)?;

            // X11 image formats:
            // 0 = Bitmap (1 bit per pixel)
            // 1 = XYPixmap (planar format)
            // 2 = ZPixmap (packed pixels - most common)

            match format {
                2 => {
                    // ZPixmap format - packed pixels (RGBA/RGB)
                    // Create a BITMAPINFO structure for the image
                    let bits_per_pixel = if depth == 1 {
                        1
                    } else if depth <= 8 {
                        8
                    } else if depth <= 16 {
                        16
                    } else if depth <= 24 {
                        24
                    } else {
                        32
                    };

                    let mut bmi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: width as i32,
                            biHeight: -(height as i32), // Negative for top-down bitmap
                            biPlanes: 1,
                            biBitCount: bits_per_pixel,
                            biCompression: BI_RGB,
                            biSizeImage: 0,
                            biXPelsPerMeter: 0,
                            biYPelsPerMeter: 0,
                            biClrUsed: 0,
                            biClrImportant: 0,
                        },
                        bmiColors: [RGBQUAD {
                            rgbBlue: 0,
                            rgbGreen: 0,
                            rgbRed: 0,
                            rgbReserved: 0,
                        }],
                    };

                    // Draw the image using SetDIBitsToDevice
                    SetDIBitsToDevice(
                        hdc,
                        dst_x as i32,
                        dst_y as i32,
                        width as u32,
                        height as u32,
                        0,
                        0,
                        0,
                        height as u32,
                        data.as_ptr() as *const _,
                        &bmi as *const _,
                        DIB_RGB_COLORS,
                    );

                    Ok(())
                }
                0 | 1 => {
                    // Bitmap or XYPixmap formats - used for cursor/stipple patterns
                    // For now, just log and continue - cursor patterns won't display correctly
                    log::debug!(
                        "PutImage format {} ({}): ignoring {} bytes for {}x{} image to drawable {:?}",
                        format,
                        if format == 0 { "Bitmap" } else { "XYPixmap" },
                        data.len(),
                        width,
                        height,
                        drawable
                    );
                    Ok(())
                }
                _ => Err(format!("Unknown image format: {}", format).into()),
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
        format: u8,
    ) -> BackendResult<(u8, u32, Vec<u8>)> {
        unsafe {
            let hdc = self.get_dc(drawable)?;
            let depth = 24u8;
            let visual = 0x21u32; // TrueColor visual

            match format {
                2 => {
                    // ZPixmap format - return 32bpp RGBA data
                    let mut bmi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: width as i32,
                            biHeight: -(height as i32), // Negative for top-down
                            biPlanes: 1,
                            biBitCount: 32, // Always return 32bpp
                            biCompression: BI_RGB,
                            biSizeImage: 0,
                            biXPelsPerMeter: 0,
                            biYPelsPerMeter: 0,
                            biClrUsed: 0,
                            biClrImportant: 0,
                        },
                        bmiColors: [RGBQUAD {
                            rgbBlue: 0,
                            rgbGreen: 0,
                            rgbRed: 0,
                            rgbReserved: 0,
                        }],
                    };

                    // Allocate buffer for pixel data (4 bytes per pixel for 32bpp)
                    let buffer_size = (width as usize) * (height as usize) * 4;
                    let mut buffer = vec![0u8; buffer_size];

                    // Create a compatible bitmap to read from
                    let hbitmap = CreateCompatibleBitmap(hdc, width as i32, height as i32);
                    let mem_dc = CreateCompatibleDC(hdc);
                    let old_bitmap = SelectObject(mem_dc, hbitmap);

                    // Copy the area from source DC to memory DC
                    BitBlt(
                        mem_dc,
                        0,
                        0,
                        width as i32,
                        height as i32,
                        hdc,
                        x as i32,
                        y as i32,
                        SRCCOPY,
                    );

                    // Read the pixels from the bitmap
                    let result = GetDIBits(
                        mem_dc,
                        hbitmap,
                        0,
                        height as u32,
                        buffer.as_mut_ptr() as *mut _,
                        &mut bmi as *mut _,
                        DIB_RGB_COLORS,
                    );

                    // Cleanup
                    SelectObject(mem_dc, old_bitmap);
                    DeleteObject(hbitmap);
                    DeleteDC(mem_dc);

                    if result == 0 {
                        return Err("GetDIBits failed".into());
                    }

                    Ok((depth, visual, buffer))
                }
                0 | 1 => {
                    // Bitmap or XYPixmap formats
                    Err(format!("Image format {} not yet implemented", format).into())
                }
                _ => Err(format!("Unknown image format: {}", format).into()),
            }
        }
    }

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        unsafe {
            let mut msg: MSG = mem::zeroed();

            // Process all available messages without blocking
            while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                // Convert Windows messages to Backend events
                match msg.message {
                    WM_PAINT => {
                        // Find which window this is for
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            self.event_queue.push(BackendEvent::Expose {
                                window: BackendWindow(*id),
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                            });
                        }
                    }
                    WM_SIZE => {
                        if let Some((id, data)) = self
                            .windows
                            .iter_mut()
                            .find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let width = (msg.lParam & 0xffff) as u16;
                            let height = ((msg.lParam >> 16) & 0xffff) as u16;

                            if width > 0 && height > 0 {
                                // Resize backing store
                                let new_bitmap =
                                    CreateCompatibleBitmap(data.hdc, width as i32, height as i32);
                                if new_bitmap != 0 {
                                    let new_dc = CreateCompatibleDC(data.hdc);
                                    if new_dc != 0 {
                                        let new_old = SelectObject(new_dc, new_bitmap as isize);
                                        // Fill with white then copy old content
                                        let bg_rect = RECT {
                                            left: 0,
                                            top: 0,
                                            right: width as i32,
                                            bottom: height as i32,
                                        };
                                        FillRect(
                                            new_dc,
                                            &bg_rect,
                                            GetStockObject(WHITE_BRUSH) as HBRUSH,
                                        );
                                        BitBlt(
                                            new_dc,
                                            0,
                                            0,
                                            data.width as i32,
                                            data.height as i32,
                                            data.mem_dc,
                                            0,
                                            0,
                                            SRCCOPY,
                                        );
                                        // Clean up old backing store
                                        SelectObject(data.mem_dc, data.old_bitmap);
                                        DeleteObject(data.mem_bitmap as isize);
                                        DeleteDC(data.mem_dc);
                                        // Update to new backing store
                                        data.mem_dc = new_dc;
                                        data.mem_bitmap = new_bitmap;
                                        data.old_bitmap = new_old;
                                        // Update GWLP_USERDATA
                                        SetWindowLongPtrW(
                                            data.hwnd,
                                            GWLP_USERDATA,
                                            new_dc as isize,
                                        );
                                    } else {
                                        DeleteObject(new_bitmap as isize);
                                    }
                                }
                            }

                            data.width = width;
                            data.height = height;

                            self.event_queue.push(BackendEvent::Configure {
                                window: BackendWindow(*id),
                                x: 0,
                                y: 0,
                                width,
                                height,
                            });
                        }
                    }
                    WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let button = match msg.message {
                                WM_LBUTTONDOWN => 1,
                                WM_RBUTTONDOWN => 3,
                                WM_MBUTTONDOWN => 2,
                                _ => 1,
                            };
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            self.event_queue.push(BackendEvent::ButtonPress {
                                window: BackendWindow(*id),
                                button,
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let button = match msg.message {
                                WM_LBUTTONUP => 1,
                                WM_RBUTTONUP => 3,
                                WM_MBUTTONUP => 2,
                                _ => 1,
                            };
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            self.event_queue.push(BackendEvent::ButtonRelease {
                                window: BackendWindow(*id),
                                button,
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_MOUSEMOVE => {
                        if let Some((id, data)) = self
                            .windows
                            .iter_mut()
                            .find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;
                            let window = BackendWindow(*id);

                            // Check if mouse just entered
                            if !data.mouse_inside {
                                data.mouse_inside = true;

                                // Track mouse leave events
                                let mut tme: TRACKMOUSEEVENT = mem::zeroed();
                                tme.cbSize = mem::size_of::<TRACKMOUSEEVENT>() as u32;
                                tme.dwFlags = TME_LEAVE;
                                tme.hwndTrack = msg.hwnd;
                                TrackMouseEvent(&mut tme);

                                self.event_queue.push(BackendEvent::EnterNotify {
                                    window,
                                    x,
                                    y,
                                    time: msg.time,
                                });
                            }

                            self.event_queue.push(BackendEvent::MotionNotify {
                                window,
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_MOUSELEAVE => {
                        if let Some((id, data)) = self
                            .windows
                            .iter_mut()
                            .find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            if data.mouse_inside {
                                data.mouse_inside = false;
                                self.event_queue.push(BackendEvent::LeaveNotify {
                                    window: BackendWindow(*id),
                                    x: 0,
                                    y: 0,
                                    time: msg.time,
                                });
                            }
                        }
                    }
                    WM_SETCURSOR => {
                        // Set the window's cursor when mouse is in client area
                        if (msg.lParam & 0xffff) as u32 == HTCLIENT {
                            if let Some((_, data)) =
                                self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                            {
                                SetCursor(data.cursor);
                            }
                        }
                    }
                    WM_KEYDOWN => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            self.event_queue.push(BackendEvent::KeyPress {
                                window: BackendWindow(*id),
                                keycode: msg.wParam as u8,
                                state: 0,
                                time: msg.time,
                                x: 0,
                                y: 0,
                            });
                        }
                    }
                    WM_KEYUP => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            self.event_queue.push(BackendEvent::KeyRelease {
                                window: BackendWindow(*id),
                                keycode: msg.wParam as u8,
                                state: 0,
                                time: msg.time,
                                x: 0,
                                y: 0,
                            });
                        }
                    }
                    WM_SETFOCUS => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            self.event_queue.push(BackendEvent::FocusIn {
                                window: BackendWindow(*id),
                            });
                        }
                    }
                    WM_KILLFOCUS => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            self.event_queue.push(BackendEvent::FocusOut {
                                window: BackendWindow(*id),
                            });
                        }
                    }
                    _ => {}
                }
            }

            // Return accumulated events
            Ok(std::mem::take(&mut self.event_queue))
        }
    }

    fn create_standard_cursor(
        &mut self,
        cursor_shape: StandardCursor,
    ) -> BackendResult<BackendCursor> {
        unsafe {
            let idc = Self::standard_cursor_to_idc(cursor_shape);
            let hcursor = LoadCursorW(0, idc);
            if hcursor == 0 {
                return Err("Failed to load cursor".into());
            }

            let id = self.next_resource_id;
            self.next_resource_id += 1;
            self.cursors.insert(id, hcursor);
            Ok(BackendCursor(id))
        }
    }

    fn free_cursor(&mut self, cursor: BackendCursor) -> BackendResult<()> {
        // System cursors don't need to be freed, just remove from our map
        self.cursors.remove(&cursor.0);
        Ok(())
    }

    fn set_window_cursor(
        &mut self,
        window: BackendWindow,
        cursor: BackendCursor,
    ) -> BackendResult<()> {
        unsafe {
            let hcursor = if cursor == BackendCursor::NONE {
                // Use default arrow cursor
                LoadCursorW(0, IDC_ARROW)
            } else {
                match self.cursors.get(&cursor.0) {
                    Some(&c) => c,
                    None => return Err("Invalid cursor handle".into()),
                }
            };

            if let Some(window_data) = self.windows.get_mut(&window.0) {
                window_data.cursor = hcursor;
                // Set the cursor immediately if the mouse is in the window
                if window_data.mouse_inside {
                    SetCursor(hcursor);
                }
            }
            Ok(())
        }
    }

    fn flush(&mut self) -> BackendResult<()> {
        unsafe {
            // Flush GDI queue to ensure all drawing operations are complete
            GdiFlush();
            // Blit backing store to window for all windows
            for data in self.windows.values() {
                BitBlt(
                    data.hdc,
                    0,
                    0,
                    data.width as i32,
                    data.height as i32,
                    data.mem_dc,
                    0,
                    0,
                    SRCCOPY,
                );
            }
            Ok(())
        }
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        unsafe {
            loop {
                // First check if we have queued events
                if let Some(event) = self.event_queue.pop() {
                    return Ok(event);
                }

                // Wait for a message (blocking)
                let mut msg: MSG = mem::zeroed();
                let ret = GetMessageW(&mut msg, 0, 0, 0);

                if ret == 0 || ret == -1 {
                    return Err("GetMessage failed or received WM_QUIT".into());
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                // Process the message and add to queue
                match msg.message {
                    WM_PAINT => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            return Ok(BackendEvent::Expose {
                                window: BackendWindow(*id),
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                            });
                        }
                    }
                    WM_SIZE => {
                        if let Some((id, data)) = self
                            .windows
                            .iter_mut()
                            .find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let width = (msg.lParam & 0xffff) as u16;
                            let height = ((msg.lParam >> 16) & 0xffff) as u16;

                            if width > 0 && height > 0 {
                                // Resize backing store
                                let new_bitmap =
                                    CreateCompatibleBitmap(data.hdc, width as i32, height as i32);
                                if new_bitmap != 0 {
                                    let new_dc = CreateCompatibleDC(data.hdc);
                                    if new_dc != 0 {
                                        let new_old = SelectObject(new_dc, new_bitmap as isize);
                                        let bg_rect = RECT {
                                            left: 0,
                                            top: 0,
                                            right: width as i32,
                                            bottom: height as i32,
                                        };
                                        FillRect(
                                            new_dc,
                                            &bg_rect,
                                            GetStockObject(WHITE_BRUSH) as HBRUSH,
                                        );
                                        BitBlt(
                                            new_dc,
                                            0,
                                            0,
                                            data.width as i32,
                                            data.height as i32,
                                            data.mem_dc,
                                            0,
                                            0,
                                            SRCCOPY,
                                        );
                                        SelectObject(data.mem_dc, data.old_bitmap);
                                        DeleteObject(data.mem_bitmap as isize);
                                        DeleteDC(data.mem_dc);
                                        data.mem_dc = new_dc;
                                        data.mem_bitmap = new_bitmap;
                                        data.old_bitmap = new_old;
                                        SetWindowLongPtrW(
                                            data.hwnd,
                                            GWLP_USERDATA,
                                            new_dc as isize,
                                        );
                                    } else {
                                        DeleteObject(new_bitmap as isize);
                                    }
                                }
                            }

                            data.width = width;
                            data.height = height;

                            return Ok(BackendEvent::Configure {
                                window: BackendWindow(*id),
                                x: 0,
                                y: 0,
                                width,
                                height,
                            });
                        }
                    }
                    WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let button = match msg.message {
                                WM_LBUTTONDOWN => 1,
                                WM_RBUTTONDOWN => 3,
                                WM_MBUTTONDOWN => 2,
                                _ => 1,
                            };
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            return Ok(BackendEvent::ButtonPress {
                                window: BackendWindow(*id),
                                button,
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let button = match msg.message {
                                WM_LBUTTONUP => 1,
                                WM_RBUTTONUP => 3,
                                WM_MBUTTONUP => 2,
                                _ => 1,
                            };
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            return Ok(BackendEvent::ButtonRelease {
                                window: BackendWindow(*id),
                                button,
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_MOUSEMOVE => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            return Ok(BackendEvent::MotionNotify {
                                window: BackendWindow(*id),
                                state: 0,
                                time: msg.time,
                                x,
                                y,
                            });
                        }
                    }
                    WM_KEYDOWN => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            return Ok(BackendEvent::KeyPress {
                                window: BackendWindow(*id),
                                keycode: msg.wParam as u8,
                                state: 0,
                                time: msg.time,
                                x: 0,
                                y: 0,
                            });
                        }
                    }
                    WM_KEYUP => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            return Ok(BackendEvent::KeyRelease {
                                window: BackendWindow(*id),
                                keycode: msg.wParam as u8,
                                state: 0,
                                time: msg.time,
                                x: 0,
                                y: 0,
                            });
                        }
                    }
                    WM_SETFOCUS => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            return Ok(BackendEvent::FocusIn {
                                window: BackendWindow(*id),
                            });
                        }
                    }
                    WM_KILLFOCUS => {
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            return Ok(BackendEvent::FocusOut {
                                window: BackendWindow(*id),
                            });
                        }
                    }
                    WM_SETCURSOR => {
                        // Set the window's cursor when mouse is in client area
                        if (msg.lParam & 0xffff) as u32 == HTCLIENT {
                            if let Some((_, data)) =
                                self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                            {
                                SetCursor(data.cursor);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn list_system_fonts(&mut self) -> BackendResult<Vec<BackendFontInfo>> {
        let mut fonts: Vec<BackendFontInfo> = Vec::new();

        unsafe {
            let hdc = GetDC(0);
            if hdc == 0 {
                return Ok(fonts);
            }

            // Set up LOGFONTW for enumeration - use DEFAULT_CHARSET to get all fonts
            let mut lf: LOGFONTW = mem::zeroed();
            lf.lfCharSet = DEFAULT_CHARSET as u8;

            // Callback data structure
            struct CallbackData {
                fonts: Vec<BackendFontInfo>,
                hdc: HDC,
            }

            let mut callback_data = CallbackData {
                fonts: Vec::new(),
                hdc,
            };

            // Font enumeration callback
            unsafe extern "system" fn enum_font_callback(
                lpelfe: *const LOGFONTW,
                lpntme: *const TEXTMETRICW,
                font_type: u32,
                lparam: LPARAM,
            ) -> i32 {
                let data = &mut *(lparam as *mut CallbackData);
                let lf = &*lpelfe;
                let tm = &*lpntme;

                // Skip raster fonts (we want TrueType/OpenType)
                if font_type & TRUETYPE_FONTTYPE == 0 {
                    return 1; // Continue enumeration
                }

                // Get font family name from LOGFONTW
                let family_name: String = lf
                    .lfFaceName
                    .iter()
                    .take_while(|&&c| c != 0)
                    .map(|&c| char::from_u32(c as u32).unwrap_or('?'))
                    .collect();

                // Skip fonts starting with @ (vertical fonts)
                if family_name.starts_with('@') {
                    return 1;
                }

                // Determine weight string
                let weight = match lf.lfWeight {
                    w if w <= 100 => "thin",
                    w if w <= 200 => "extralight",
                    w if w <= 300 => "light",
                    w if w <= 400 => "medium",
                    w if w <= 500 => "medium",
                    w if w <= 600 => "demibold",
                    w if w <= 700 => "bold",
                    w if w <= 800 => "extrabold",
                    _ => "black",
                };

                // Determine slant
                let slant = if lf.lfItalic != 0 { "i" } else { "r" };

                // Calculate pixel size from height (can be negative for character height)
                let pixel_size = if lf.lfHeight < 0 {
                    (-lf.lfHeight) as u16
                } else {
                    lf.lfHeight as u16
                };

                // Point size in decipoints (pixel * 720 / 96 for 96 DPI)
                let point_size = if pixel_size > 0 {
                    ((pixel_size as u32 * 720) / 96) as u16
                } else {
                    120 // Default 12pt
                };

                // Character width (0 for proportional, actual width for fixed-pitch)
                let char_width = if lf.lfPitchAndFamily & FIXED_PITCH as u8 != 0 {
                    tm.tmAveCharWidth as u16
                } else {
                    0
                };

                // Determine charset registry/encoding
                let (registry, encoding) = match lf.lfCharSet {
                    ANSI_CHARSET => ("iso8859", "1"),
                    SYMBOL_CHARSET => ("adobe", "fontspecific"),
                    SHIFTJIS_CHARSET => ("jisx0208.1983", "0"),
                    HANGEUL_CHARSET => ("ksc5601.1987", "0"),
                    GB2312_CHARSET => ("gb2312.1980", "0"),
                    CHINESEBIG5_CHARSET => ("big5", "0"),
                    GREEK_CHARSET => ("iso8859", "7"),
                    TURKISH_CHARSET => ("iso8859", "9"),
                    HEBREW_CHARSET => ("iso8859", "8"),
                    ARABIC_CHARSET => ("iso8859", "6"),
                    BALTIC_CHARSET => ("iso8859", "13"),
                    RUSSIAN_CHARSET => ("koi8", "r"),
                    THAI_CHARSET => ("tis620.2533", "0"),
                    EASTEUROPE_CHARSET => ("iso8859", "2"),
                    _ => ("iso10646", "1"), // Unicode fallback
                };

                let font_info = BackendFontInfo {
                    xlfd_name: String::new(), // Will be generated
                    family: family_name.clone(),
                    weight: weight.to_string(),
                    slant: slant.to_string(),
                    pixel_size,
                    point_size,
                    char_width,
                    ascent: tm.tmAscent as i16,
                    descent: tm.tmDescent as i16,
                    registry: registry.to_string(),
                    encoding: encoding.to_string(),
                };

                // Generate XLFD name
                let mut font_with_xlfd = font_info;
                font_with_xlfd.xlfd_name = font_with_xlfd.generate_xlfd();

                data.fonts.push(font_with_xlfd);

                1 // Continue enumeration
            }

            // Enumerate fonts
            EnumFontFamiliesExW(
                hdc,
                &lf,
                Some(enum_font_callback),
                &mut callback_data as *mut CallbackData as LPARAM,
                0,
            );

            ReleaseDC(0, hdc);

            fonts = callback_data.fonts;
        }

        log::debug!("Windows backend: enumerated {} fonts", fonts.len());
        Ok(fonts)
    }
}
