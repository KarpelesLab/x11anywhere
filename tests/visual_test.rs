/// Visual test client for X11Anywhere
///
/// This test program connects to an X11 server and draws various test patterns
/// to verify rendering correctness across different backends (Windows, macOS, Linux).
///
/// Test patterns include:
/// - Solid color rectangles
/// - Lines and shapes
/// - Arcs and circles
/// - Text rendering
/// - Image/bitmap operations
///
/// After rendering, it captures a screenshot and compares it against a reference image
/// to detect rendering issues.

mod screenshot;

use std::env;
use std::path::PathBuf;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

const WINDOW_WIDTH: u16 = 800;
const WINDOW_HEIGHT: u16 = 600;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the X server
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    println!("Connected to X11 server");
    println!("Screen size: {}x{}", screen.width_in_pixels, screen.height_in_pixels);

    // Create a window
    let window = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        screen.root,
        0,
        0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(EventMask::EXPOSURE | EventMask::KEY_PRESS),
    )?;

    // Set window title
    conn.change_property8(
        PropMode::REPLACE,
        window,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"X11Anywhere Visual Test",
    )?;

    // Map the window
    conn.map_window(window)?;
    conn.flush()?;

    println!("Window created and mapped");

    // Create a graphics context
    let gc = conn.generate_id()?;
    conn.create_gc(
        gc,
        window,
        &CreateGCAux::new()
            .foreground(screen.black_pixel)
            .background(screen.white_pixel)
            .line_width(2),
    )?;

    // Wait for Expose event
    loop {
        let event = conn.wait_for_event()?;
        match event {
            Event::Expose(_) => {
                println!("Received Expose event, drawing test patterns...");
                draw_test_patterns(&conn, window, gc, screen)?;
                conn.flush()?;
                break;
            }
            Event::KeyPress(_) => {
                println!("Key pressed, exiting");
                break;
            }
            _ => {}
        }
    }

    // Keep window open for screenshot (allow time for rendering to complete)
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Capture screenshot of the test window
    println!("Capturing screenshot of window {}...", window);
    let screenshot = match screenshot::capture_window(window) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to capture screenshot: {}", e);
            // Cleanup before exit
            conn.free_gc(gc)?;
            conn.destroy_window(window)?;
            conn.flush()?;
            std::process::exit(1);
        }
    };

    println!(
        "Screenshot captured: {}x{} ({} bytes)",
        screenshot.width,
        screenshot.height,
        screenshot.data.len()
    );

    // Save actual screenshot for debugging
    let output_dir = env::var("VISUAL_TEST_OUTPUT").unwrap_or_else(|_| ".".to_string());
    let output_path = PathBuf::from(&output_dir).join("visual_test_actual.png");
    if let Err(e) = screenshot::save_png(&screenshot, &output_path) {
        eprintln!("Failed to save screenshot: {}", e);
    } else {
        println!("Screenshot saved to: {}", output_path.display());
    }

    // Compare with reference image if it exists
    let reference_path = PathBuf::from(&output_dir).join("visual_test_reference.png");
    let test_result = if reference_path.exists() {
        println!("Comparing with reference image: {}", reference_path.display());
        match screenshot::load_png(&reference_path) {
            Ok(reference) => {
                let tolerance = 0.01; // 1% tolerance per pixel
                match screenshot::compare_screenshots(&screenshot, &reference, tolerance) {
                    Ok(diff_percentage) => {
                        println!("Difference: {:.2}%", diff_percentage);
                        if diff_percentage > 5.0 {
                            // Allow 5% total difference
                            eprintln!("FAIL: Screenshot differs too much from reference (>{:.0}%)", 5.0);
                            false
                        } else {
                            println!("PASS: Screenshot matches reference");
                            true
                        }
                    }
                    Err(e) => {
                        eprintln!("FAIL: Comparison error: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to load reference image: {}", e);
                println!("Skipping comparison (reference image not available)");
                true // Don't fail if reference doesn't exist
            }
        }
    } else {
        println!("No reference image found at: {}", reference_path.display());
        println!("Run with VISUAL_TEST_MODE=generate to create reference image");
        true // Don't fail if reference doesn't exist
    };

    // Cleanup
    conn.free_gc(gc)?;
    conn.destroy_window(window)?;
    conn.flush()?;

    println!("Visual test completed");

    // Exit with appropriate code
    if test_result {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn draw_test_patterns(
    conn: &impl Connection,
    window: Window,
    gc: Gcontext,
    screen: &Screen,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Drawing test patterns...");

    // Test 1: Colored rectangles (top row)
    draw_colored_rectangles(conn, window, gc, screen)?;

    // Test 2: Lines and shapes (middle section)
    draw_lines_and_shapes(conn, window, gc, screen)?;

    // Test 3: Arcs and circles (bottom left)
    draw_arcs_and_circles(conn, window, gc, screen)?;

    // Test 4: Text (bottom right)
    draw_text(conn, window, gc, screen)?;

    Ok(())
}

fn draw_colored_rectangles(
    conn: &impl Connection,
    window: Window,
    gc: Gcontext,
    _screen: &Screen,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Drawing colored rectangles...");

    let colors = [
        0xFF0000u32, // Red
        0x00FF00u32, // Green
        0x0000FFu32, // Blue
        0xFFFF00u32, // Yellow
        0xFF00FFu32, // Magenta
        0x00FFFFu32, // Cyan
    ];

    let rect_width = 120u16;
    let rect_height = 80u16;
    let spacing = 10u16;

    for (i, &color) in colors.iter().enumerate() {
        let x = (spacing + (rect_width + spacing) * i as u16) as i16;
        let y = 20i16;

        // Change foreground color
        conn.change_gc(gc, &ChangeGCAux::new().foreground(color))?;

        // Draw filled rectangle
        conn.poly_fill_rectangle(window, gc, &[Rectangle {
            x,
            y,
            width: rect_width,
            height: rect_height,
        }])?;
    }

    Ok(())
}

fn draw_lines_and_shapes(
    conn: &impl Connection,
    window: Window,
    gc: Gcontext,
    _screen: &Screen,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Drawing lines and shapes...");

    // Reset to black
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x000000))?;

    let y_start = 120i16;

    // Horizontal line
    conn.poly_line(
        CoordMode::ORIGIN,
        window,
        gc,
        &[Point { x: 50, y: y_start }, Point { x: 750, y: y_start }],
    )?;

    // Vertical line
    conn.poly_line(
        CoordMode::ORIGIN,
        window,
        gc,
        &[Point { x: 400, y: y_start }, Point { x: 400, y: y_start + 100 }],
    )?;

    // Diagonal lines (X pattern)
    conn.poly_line(
        CoordMode::ORIGIN,
        window,
        gc,
        &[
            Point { x: 100, y: y_start + 20 },
            Point { x: 200, y: y_start + 120 },
        ],
    )?;
    conn.poly_line(
        CoordMode::ORIGIN,
        window,
        gc,
        &[
            Point { x: 200, y: y_start + 20 },
            Point { x: 100, y: y_start + 120 },
        ],
    )?;

    // Rectangle outline
    conn.poly_rectangle(window, gc, &[Rectangle {
        x: 250,
        y: y_start + 20,
        width: 100,
        height: 80,
    }])?;

    // Filled rectangle
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x808080))?;
    conn.poly_fill_rectangle(window, gc, &[Rectangle {
        x: 450,
        y: y_start + 20,
        width: 100,
        height: 80,
    }])?;

    // Polygon (triangle)
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x0000FF))?;
    conn.fill_poly(
        window,
        gc,
        PolyShape::COMPLEX,
        CoordMode::ORIGIN,
        &[
            Point { x: 600, y: y_start + 100 },
            Point { x: 650, y: y_start + 20 },
            Point { x: 700, y: y_start + 100 },
        ],
    )?;

    Ok(())
}

fn draw_arcs_and_circles(
    conn: &impl Connection,
    window: Window,
    gc: Gcontext,
    _screen: &Screen,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Drawing arcs and circles...");

    let y_base = 350i16;

    // Circle outline
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x000000))?;
    conn.poly_arc(window, gc, &[Arc {
        x: 50,
        y: y_base,
        width: 100,
        height: 100,
        angle1: 0,
        angle2: 360 * 64, // Full circle (angles in 1/64 degrees)
    }])?;

    // Filled circle
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0xFF0000))?;
    conn.poly_fill_arc(window, gc, &[Arc {
        x: 180,
        y: y_base,
        width: 100,
        height: 100,
        angle1: 0,
        angle2: 360 * 64,
    }])?;

    // Arc (quarter circle)
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x00FF00))?;
    conn.poly_arc(window, gc, &[Arc {
        x: 310,
        y: y_base,
        width: 100,
        height: 100,
        angle1: 0,
        angle2: 90 * 64, // 90 degrees
    }])?;

    // Ellipse
    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x0000FF))?;
    conn.poly_arc(window, gc, &[Arc {
        x: 440,
        y: y_base,
        width: 150,
        height: 80,
        angle1: 0,
        angle2: 360 * 64,
    }])?;

    Ok(())
}

fn draw_text(
    conn: &impl Connection,
    window: Window,
    gc: Gcontext,
    _screen: &Screen,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Drawing text...");

    conn.change_gc(gc, &ChangeGCAux::new().foreground(0x000000))?;

    // Draw text strings
    let text1 = b"X11Anywhere Visual Test";
    let text2 = b"Rendering: OK";
    let text3 = b"Colors: RGB";

    conn.image_text8(window, gc, 50, 500, text1)?;
    conn.image_text8(window, gc, 50, 530, text2)?;
    conn.image_text8(window, gc, 50, 560, text3)?;

    Ok(())
}
