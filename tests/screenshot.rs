/// Platform-specific screenshot capture utilities
///
/// This module provides screenshot capture functionality for Windows, macOS, and Linux.
/// Used by visual tests to capture and verify rendering output.

use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>, // RGBA format
}

/// Capture a screenshot of a specific window by ID
#[cfg(target_os = "linux")]
pub fn capture_window(window_id: u32) -> Result<Screenshot, Box<dyn std::error::Error>> {
    use std::env;
    use std::fs;

    // Use ImageMagick's import to capture specific window as PNG
    let temp_png = "/tmp/x11anywhere_visual_test_window.png";
    let window_id_hex = format!("0x{:x}", window_id);

    let output = Command::new("import")
        .arg("-window")
        .arg(&window_id_hex)
        .arg(temp_png)
        .env("DISPLAY", env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()))
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "import (ImageMagick) failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let png_data = fs::read(temp_png)?;
    let screenshot = decode_png(&png_data)?;

    // Clean up
    let _ = fs::remove_file(temp_png);

    Ok(screenshot)
}

/// Capture a screenshot of the entire screen
#[cfg(target_os = "windows")]
pub fn capture_screen() -> Result<Screenshot, Box<dyn std::error::Error>> {
    use std::mem;
    use std::ptr;

    unsafe {
        // Get screen DC
        let screen_dc = windows_sys::Win32::Graphics::Gdi::GetDC(0);
        if screen_dc == 0 {
            return Err("Failed to get screen DC".into());
        }

        // Get screen dimensions
        let width =
            windows_sys::Win32::Graphics::Gdi::GetDeviceCaps(screen_dc, windows_sys::Win32::Graphics::Gdi::HORZRES as i32);
        let height =
            windows_sys::Win32::Graphics::Gdi::GetDeviceCaps(screen_dc, windows_sys::Win32::Graphics::Gdi::VERTRES as i32);

        // Create compatible DC and bitmap
        let mem_dc = windows_sys::Win32::Graphics::Gdi::CreateCompatibleDC(screen_dc);
        let bitmap = windows_sys::Win32::Graphics::Gdi::CreateCompatibleBitmap(screen_dc, width, height);
        let old_bitmap = windows_sys::Win32::Graphics::Gdi::SelectObject(mem_dc, bitmap);

        // Copy screen to bitmap
        windows_sys::Win32::Graphics::Gdi::BitBlt(
            mem_dc,
            0,
            0,
            width,
            height,
            screen_dc,
            0,
            0,
            windows_sys::Win32::Graphics::Gdi::SRCCOPY,
        );

        // Get bitmap data
        let mut bmi: windows_sys::Win32::Graphics::Gdi::BITMAPINFO = mem::zeroed();
        bmi.bmiHeader.biSize = mem::size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // Top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = windows_sys::Win32::Graphics::Gdi::BI_RGB;

        let buffer_size = (width * height * 4) as usize;
        let mut buffer = vec![0u8; buffer_size];

        let result = windows_sys::Win32::Graphics::Gdi::GetDIBits(
            mem_dc,
            bitmap,
            0,
            height as u32,
            buffer.as_mut_ptr() as *mut _,
            &mut bmi as *mut _,
            windows_sys::Win32::Graphics::Gdi::DIB_RGB_COLORS,
        );

        // Cleanup
        windows_sys::Win32::Graphics::Gdi::SelectObject(mem_dc, old_bitmap);
        windows_sys::Win32::Graphics::Gdi::DeleteObject(bitmap);
        windows_sys::Win32::Graphics::Gdi::DeleteDC(mem_dc);
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(0, screen_dc);

        if result == 0 {
            return Err("GetDIBits failed".into());
        }

        Ok(Screenshot {
            width: width as u32,
            height: height as u32,
            data: buffer,
        })
    }
}

/// Capture a screenshot of a specific window by ID (macOS)
#[cfg(target_os = "macos")]
pub fn capture_window(_window_id: u32) -> Result<Screenshot, Box<dyn std::error::Error>> {
    // For macOS, we'd need to use CGWindowListCreateImage with the window ID
    // For now, fall back to full screen capture
    capture_screen()
}

/// Capture a screenshot of a specific window by ID (Windows)
#[cfg(target_os = "windows")]
pub fn capture_window(_window_id: u32) -> Result<Screenshot, Box<dyn std::error::Error>> {
    // For Windows, we'd need to use GetWindowDC instead of screen DC
    // For now, fall back to full screen capture
    capture_screen()
}

/// Capture a screenshot on macOS using screencapture utility
#[cfg(target_os = "macos")]
pub fn capture_screen() -> Result<Screenshot, Box<dyn std::error::Error>> {
    use std::fs;

    // Create temporary file for screenshot
    let temp_path = "/tmp/x11anywhere_visual_test.png";

    // Use screencapture utility (built into macOS)
    let output = Command::new("screencapture")
        .arg("-x") // No sound
        .arg("-T")
        .arg("0") // Capture immediately
        .arg(temp_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Read the PNG file and decode it
    let png_data = fs::read(temp_path)?;
    let screenshot = decode_png(&png_data)?;

    // Clean up temp file
    let _ = fs::remove_file(temp_path);

    Ok(screenshot)
}

/// Capture a screenshot on Linux using ImageMagick's import command
#[cfg(target_os = "linux")]
pub fn capture_screen() -> Result<Screenshot, Box<dyn std::error::Error>> {
    use std::env;
    use std::fs;

    // Use ImageMagick's import to capture root window directly as PNG
    let temp_png = "/tmp/x11anywhere_visual_test.png";

    let output = Command::new("import")
        .arg("-window")
        .arg("root")
        .arg(temp_png)
        .env("DISPLAY", env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()))
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "import (ImageMagick) failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let png_data = fs::read(temp_png)?;
    let screenshot = decode_png(&png_data)?;

    // Clean up
    let _ = fs::remove_file(temp_png);

    Ok(screenshot)
}

/// Decode PNG data to RGBA
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn decode_png(data: &[u8]) -> Result<Screenshot, Box<dyn std::error::Error>> {
    use image::ImageReader;
    use std::io::Cursor;

    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()?
        .decode()?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Screenshot {
        width,
        height,
        data: rgba.into_raw(),
    })
}

/// Save screenshot as PNG file
pub fn save_png(screenshot: &Screenshot, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(screenshot.width, screenshot.height, screenshot.data.clone())
            .ok_or("Failed to create image buffer")?;

    img.save(path)?;
    Ok(())
}

/// Load a screenshot from a PNG file
pub fn load_png(path: &Path) -> Result<Screenshot, Box<dyn std::error::Error>> {
    use image::ImageReader;

    let img = ImageReader::open(path)?.decode()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Screenshot {
        width,
        height,
        data: rgba.into_raw(),
    })
}

/// Compare two screenshots and return the difference percentage
pub fn compare_screenshots(
    actual: &Screenshot,
    expected: &Screenshot,
    tolerance: f32,
) -> Result<f32, Box<dyn std::error::Error>> {
    if actual.width != expected.width || actual.height != expected.height {
        return Err(format!(
            "Screenshot dimensions mismatch: {}x{} vs {}x{}",
            actual.width, actual.height, expected.width, expected.height
        )
        .into());
    }

    let pixel_count = (actual.width * actual.height) as usize;
    let mut diff_count = 0;

    for i in 0..pixel_count {
        let idx = i * 4;
        let r_diff = (actual.data[idx] as i32 - expected.data[idx] as i32).abs();
        let g_diff = (actual.data[idx + 1] as i32 - expected.data[idx + 1] as i32).abs();
        let b_diff = (actual.data[idx + 2] as i32 - expected.data[idx + 2] as i32).abs();

        // Calculate per-pixel difference (0-255 range per channel)
        let pixel_diff = (r_diff + g_diff + b_diff) as f32 / (255.0 * 3.0);

        if pixel_diff > tolerance {
            diff_count += 1;
        }
    }

    let diff_percentage = (diff_count as f32 / pixel_count as f32) * 100.0;
    Ok(diff_percentage)
}
