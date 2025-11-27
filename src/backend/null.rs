//! Null Backend - Minimal backend for testing
//!
//! This backend accepts all commands but doesn't perform any actual rendering.
//! It's useful for testing the protocol implementation without requiring a
//! real display system.

use super::*;
use crate::protocol::*;

pub struct NullBackend {
    next_window_id: usize,
    next_pixmap_id: usize,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            next_window_id: 1,
            next_pixmap_id: 1,
        }
    }
}

impl Backend for NullBackend {
    fn init(&mut self) -> BackendResult<()> {
        Ok(())
    }

    fn get_screen_info(&self) -> BackendResult<ScreenInfo> {
        Ok(ScreenInfo {
            width: 1920,
            height: 1080,
            width_mm: 508,
            height_mm: 285,
            root_visual: VisualID::new(0x21),
            root_depth: 24,
            white_pixel: 0xFFFFFF,
            black_pixel: 0x000000,
        })
    }

    fn get_visuals(&self) -> BackendResult<Vec<VisualInfo>> {
        Ok(vec![VisualInfo {
            visual_id: VisualID::new(0x21),
            class: 4, // TrueColor
            bits_per_rgb: 8,
            colormap_entries: 256,
            red_mask: 0xFF0000,
            green_mask: 0x00FF00,
            blue_mask: 0x0000FF,
        }])
    }

    fn create_window(&mut self, _params: WindowParams) -> BackendResult<BackendWindow> {
        let id = self.next_window_id;
        self.next_window_id += 1;
        Ok(BackendWindow(id))
    }

    fn destroy_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn map_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn unmap_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn configure_window(
        &mut self,
        _window: BackendWindow,
        _config: WindowConfig,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn raise_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn lower_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn set_window_title(&mut self, _window: BackendWindow, _title: &str) -> BackendResult<()> {
        Ok(())
    }

    fn clear_area(
        &mut self,
        _window: BackendWindow,
        _x: i16,
        _y: i16,
        _width: u16,
        _height: u16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_rectangle(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _width: u16,
        _height: u16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn fill_rectangle(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _width: u16,
        _height: u16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_line(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x1: i16,
        _y1: i16,
        _x2: i16,
        _y2: i16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_points(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _points: &[Point],
    ) -> BackendResult<()> {
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
        Ok(())
    }

    fn draw_arcs(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _arcs: &[Arc],
    ) -> BackendResult<()> {
        Ok(())
    }

    fn fill_arcs(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _arcs: &[Arc],
    ) -> BackendResult<()> {
        Ok(())
    }

    fn fill_polygon(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _points: &[Point],
    ) -> BackendResult<()> {
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
        Ok(())
    }

    fn create_pixmap(&mut self, _width: u16, _height: u16, _depth: u8) -> BackendResult<usize> {
        let id = self.next_pixmap_id;
        self.next_pixmap_id += 1;
        Ok(id)
    }

    fn free_pixmap(&mut self, _pixmap: usize) -> BackendResult<()> {
        Ok(())
    }

    fn put_image(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _width: u16,
        _height: u16,
        _dst_x: i16,
        _dst_y: i16,
        _depth: u8,
        _format: u8,
        _data: &[u8],
    ) -> BackendResult<()> {
        Ok(())
    }

    fn get_image(
        &mut self,
        _drawable: BackendDrawable,
        _x: i16,
        _y: i16,
        width: u16,
        height: u16,
        _plane_mask: u32,
        _format: u8,
    ) -> BackendResult<(u8, u32, Vec<u8>)> {
        // Return a blank image (black) with depth 24 and visual 0x21
        let size = (width as usize) * (height as usize) * 4; // RGBA
        Ok((24, 0x21, vec![0u8; size]))
    }

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        Ok(vec![])
    }

    fn flush(&mut self) -> BackendResult<()> {
        Ok(())
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        // Just sleep to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(100));
        Err("No events available".into())
    }

    fn list_system_fonts(&mut self) -> BackendResult<Vec<BackendFontInfo>> {
        // Return a minimal set of test fonts for the null backend
        Ok(vec![
            BackendFontInfo {
                xlfd_name: "-misc-fixed-medium-r-normal--13-120-75-75-c-80-iso8859-1".to_string(),
                family: "fixed".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 13,
                point_size: 120,
                char_width: 80,
                ascent: 10,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-misc-fixed-bold-r-normal--13-120-75-75-c-80-iso8859-1".to_string(),
                family: "fixed".to_string(),
                weight: "bold".to_string(),
                slant: "r".to_string(),
                pixel_size: 13,
                point_size: 120,
                char_width: 80,
                ascent: 10,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
            BackendFontInfo {
                xlfd_name: "-*-helvetica-medium-r-normal--12-120-75-75-p-67-iso8859-1".to_string(),
                family: "helvetica".to_string(),
                weight: "medium".to_string(),
                slant: "r".to_string(),
                pixel_size: 12,
                point_size: 120,
                char_width: 0, // proportional
                ascent: 9,
                descent: 3,
                registry: "iso8859".to_string(),
                encoding: "1".to_string(),
            },
        ])
    }
}
