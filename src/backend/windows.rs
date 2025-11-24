//! Windows Backend - Using Win32 APIs
//!
//! This backend provides X11 protocol support on Windows using native
//! Win32 APIs and GDI for drawing. It translates X11 window operations
//! and drawing commands to their Windows equivalents.

use super::*;
use crate::protocol::{self, *};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter::once;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
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
    width: u16,
    height: u16,
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
                let hdc = BeginPaint(hwnd, &mut ps);
                // Actual painting is done via X11 drawing commands
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

    /// Get device context for drawable
    fn get_dc(&self, drawable: BackendDrawable) -> Result<HDC, String> {
        match drawable {
            BackendDrawable::Window(win) => {
                let data = self.get_window_data(win)?;
                Ok(data.hdc)
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

            // Create the window
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                window_name.as_ptr(),
                style,
                params.x as i32,
                params.y as i32,
                params.width as i32,
                params.height as i32,
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

            let id = self.next_resource_id;
            self.next_resource_id += 1;

            self.windows.insert(
                id,
                WindowData {
                    hwnd,
                    hdc,
                    width: params.width,
                    height: params.height,
                },
            );

            Ok(BackendWindow(id))
        }
    }

    fn destroy_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            if let Some(data) = self.windows.remove(&window.0) {
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
            Ok(())
        }
    }

    fn unmap_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        unsafe {
            let hwnd = self.get_hwnd(window)?;
            ShowWindow(hwnd, SW_HIDE);
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
            let mut rect: RECT = mem::zeroed();
            GetWindowRect(hwnd, &mut rect);

            let x = config.x.unwrap_or(rect.left as i16) as i32;
            let y = config.y.unwrap_or(rect.top as i16) as i32;
            let width = config.width.unwrap_or((rect.right - rect.left) as u16) as i32;
            let height = config.height.unwrap_or((rect.bottom - rect.top) as u16) as i32;

            let flags = SWP_NOZORDER | SWP_NOACTIVATE;
            SetWindowPos(hwnd, 0, x, y, width, height, flags);

            // Update stored dimensions
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
            let hdc = data.hdc;

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

            let brush = self.create_brush(gc);
            let rect = RECT {
                left: x as i32,
                top: y as i32,
                right: x as i32 + width as i32,
                bottom: y as i32 + height as i32,
            };

            FillRect(hdc, &rect, brush);
            DeleteObject(brush as isize);

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
                    // Bitmap or XYPixmap formats - not commonly used, stub for now
                    Err(format!("Image format {} not yet implemented", format).into())
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
    ) -> BackendResult<Vec<u8>> {
        unsafe {
            let hdc = self.get_dc(drawable)?;

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

                    Ok(buffer)
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
                        if let Some((id, _)) =
                            self.windows.iter().find(|(_, data)| data.hwnd == msg.hwnd)
                        {
                            let x = (msg.lParam & 0xffff) as i16;
                            let y = ((msg.lParam >> 16) & 0xffff) as i16;

                            self.event_queue.push(BackendEvent::MotionNotify {
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

    fn flush(&mut self) -> BackendResult<()> {
        unsafe {
            // Flush GDI queue
            GdiFlush();
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
                    _ => {}
                }
            }
        }
    }
}
