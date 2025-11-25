/// Visual test client for X11Anywhere using raw X11 protocol
///
/// This test connects to the X11 server using raw TCP/IP sockets and sends
/// X11 protocol commands directly. No platform-specific libraries required.
mod screenshot;

use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

const WINDOW_WIDTH: u16 = 800;
const WINDOW_HEIGHT: u16 = 600;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse DISPLAY environment variable
    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let display_num = parse_display(&display)?;

    // Connect to X server
    let addr = format!("127.0.0.1:{}", 6000 + display_num);
    println!("Connecting to X11 server at {}...", addr);
    let mut stream = TcpStream::connect(&addr)?;

    // Send connection setup
    send_connection_setup(&mut stream)?;

    // Read setup response
    let (root_window, root_visual, screen_width, screen_height) = read_setup_response(&mut stream)?;
    println!(
        "Connected! Screen: {}x{}, Root: {}",
        screen_width, screen_height, root_window
    );

    // Create window
    let window_id = 0x02000001u32; // Use a fixed ID
    create_window(&mut stream, window_id, root_window, root_visual)?;
    println!("Window created: {}", window_id);

    // Map window
    map_window(&mut stream, window_id)?;
    println!("Window mapped");

    // Create GC
    let gc_id = 0x02000002u32;
    create_gc(&mut stream, gc_id, window_id)?;
    println!("GC created: {}", gc_id);

    // Open a font for text rendering
    let font_id = 0x02000003u32;
    if let Err(e) = open_font(&mut stream, font_id, "fixed") {
        println!("Warning: Could not open font: {}", e);
    }

    // Draw test patterns
    println!("Drawing test patterns...");
    draw_colored_rectangles(&mut stream, window_id, gc_id)?;
    draw_lines(&mut stream, window_id, gc_id)?;
    draw_rectangle_outlines(&mut stream, window_id, gc_id)?;
    draw_arcs(&mut stream, window_id, gc_id)?;
    draw_filled_arcs(&mut stream, window_id, gc_id)?;
    draw_polygon(&mut stream, window_id, gc_id)?;
    draw_points(&mut stream, window_id, gc_id)?;
    draw_segments(&mut stream, window_id, gc_id)?;
    draw_text_test(&mut stream, window_id, gc_id, font_id)?;

    // Wait for rendering - give extra time for compositor to update
    // macOS needs more time due to async dispatch to main thread
    #[cfg(target_os = "macos")]
    std::thread::sleep(std::time::Duration::from_millis(3000));
    #[cfg(not(target_os = "macos"))]
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Capture screenshot (full screen since window IDs don't match between client and server)
    println!("Capturing screenshot...");
    let screenshot = screenshot::capture_screen()?;
    println!(
        "Screenshot captured: {}x{} ({} bytes)",
        screenshot.width,
        screenshot.height,
        screenshot.data.len()
    );

    // Save screenshot
    let output_dir = env::var("VISUAL_TEST_OUTPUT").unwrap_or_else(|_| ".".to_string());
    let output_path = PathBuf::from(&output_dir).join("visual_test_actual.png");
    screenshot::save_png(&screenshot, &output_path)?;
    println!("Screenshot saved to: {}", output_path.display());

    // Validate that rectangles were actually rendered
    println!("Validating rendered rectangles...");
    let validation_result = validate_rectangles(&screenshot);

    if !validation_result {
        eprintln!("FAIL: Rectangle validation failed - rectangles not rendered correctly");
        std::process::exit(1);
    }
    println!("PASS: All rectangles validated successfully");

    // Compare with reference if available
    let reference_path = PathBuf::from(&output_dir).join("visual_test_reference.png");
    let test_result = if reference_path.exists() {
        println!(
            "Comparing with reference image: {}",
            reference_path.display()
        );
        match screenshot::load_png(&reference_path) {
            Ok(reference) => {
                let tolerance = 0.01;
                match screenshot::compare_screenshots(&screenshot, &reference, tolerance) {
                    Ok(diff_percentage) => {
                        println!("Difference: {:.2}%", diff_percentage);
                        if diff_percentage > 5.0 {
                            eprintln!("FAIL: Screenshot differs too much from reference (>5%)");
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
                println!("Skipping comparison");
                true
            }
        }
    } else {
        println!("No reference image found at: {}", reference_path.display());
        println!("Run with VISUAL_TEST_MODE=generate to create reference image");
        true
    };

    println!("Visual test completed");
    if !test_result {
        std::process::exit(1);
    }
    Ok(())
}

fn parse_display(display: &str) -> Result<u16, Box<dyn std::error::Error>> {
    // Parse :N or host:N format
    let parts: Vec<&str> = display.split(':').collect();
    let num_str = parts.last().ok_or("Invalid DISPLAY")?;
    // Remove screen number if present (.0, .1, etc)
    let num_str = num_str.split('.').next().unwrap_or(num_str);
    let num: u16 = num_str.parse()?;
    Ok(num)
}

fn send_connection_setup(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // Connection setup: LSB first, protocol 11.0, no auth
    let mut setup = Vec::new();
    setup.push(b'l'); // LSB first
    setup.push(0); // Padding
    setup.extend_from_slice(&11u16.to_le_bytes()); // Protocol major
    setup.extend_from_slice(&0u16.to_le_bytes()); // Protocol minor
    setup.extend_from_slice(&0u16.to_le_bytes()); // Auth proto name length
    setup.extend_from_slice(&0u16.to_le_bytes()); // Auth proto data length
    setup.extend_from_slice(&[0, 0]); // Padding

    stream.write_all(&setup)?;
    stream.flush()?;
    Ok(())
}

fn read_setup_response(
    stream: &mut TcpStream,
) -> Result<(u32, u32, u16, u16), Box<dyn std::error::Error>> {
    // Read status byte
    let mut status = [0u8; 1];
    stream.read_exact(&mut status)?;

    if status[0] != 1 {
        return Err("Connection setup failed".into());
    }

    // Read fixed header (39 bytes after status)
    // Format: unused(1), protocol-major(2), protocol-minor(2), length(2),
    //         release(4), resource-id-base(4), resource-id-mask(4),
    //         motion-buffer-size(4), vendor-len(2), max-request-len(2),
    //         num-screens(1), num-formats(1), image-byte-order(1),
    //         bitmap-format-bit-order(1), scanline-unit(1), scanline-pad(1),
    //         min-keycode(1), max-keycode(1), unused(4)
    let mut header = vec![0u8; 39];
    stream.read_exact(&mut header)?;

    // Parse header fields
    let vendor_len = u16::from_le_bytes([header[23], header[24]]) as usize;
    let num_screens = header[27] as usize;
    let num_formats = header[28] as usize;

    // Read vendor string (padded to 4 bytes)
    let vendor_padded = (vendor_len + 3) & !3;
    let mut vendor = vec![0u8; vendor_padded];
    stream.read_exact(&mut vendor)?;

    // Read pixmap formats (8 bytes each)
    let mut formats = vec![0u8; num_formats * 8];
    stream.read_exact(&mut formats)?;

    // Read first screen data
    // Screen format: root(4), default-colormap(4), white-pixel(4), black-pixel(4),
    //                current-input-masks(4), width-in-pixels(2), height-in-pixels(2),
    //                width-in-mm(2), height-in-mm(2), min-installed-maps(2),
    //                max-installed-maps(2), root-visual(4), backing-stores(1),
    //                save-unders(1), root-depth(1), num-depths(1)
    let mut screen_data = vec![0u8; 40];
    stream.read_exact(&mut screen_data)?;

    // Parse screen data
    let root_window = u32::from_le_bytes([
        screen_data[0],
        screen_data[1],
        screen_data[2],
        screen_data[3],
    ]);
    let screen_width = u16::from_le_bytes([screen_data[20], screen_data[21]]);
    let screen_height = u16::from_le_bytes([screen_data[22], screen_data[23]]);
    let root_visual = u32::from_le_bytes([
        screen_data[32],
        screen_data[33],
        screen_data[34],
        screen_data[35],
    ]);

    // Read remaining depth/visual data (we just need to consume it)
    let num_depths = screen_data[39] as usize;
    for _ in 0..num_depths {
        // Depth header: depth(1), unused(1), num-visuals(2), unused(4)
        let mut depth_header = vec![0u8; 8];
        stream.read_exact(&mut depth_header)?;
        let num_visuals = u16::from_le_bytes([depth_header[2], depth_header[3]]) as usize;
        // Each visual is 24 bytes
        let mut visuals = vec![0u8; num_visuals * 24];
        stream.read_exact(&mut visuals)?;
    }

    // Skip remaining screens if any
    for _ in 1..num_screens {
        let mut extra_screen = vec![0u8; 40];
        stream.read_exact(&mut extra_screen)?;
        let extra_depths = extra_screen[39] as usize;
        for _ in 0..extra_depths {
            let mut depth_header = vec![0u8; 8];
            stream.read_exact(&mut depth_header)?;
            let num_visuals = u16::from_le_bytes([depth_header[2], depth_header[3]]) as usize;
            let mut visuals = vec![0u8; num_visuals * 24];
            stream.read_exact(&mut visuals)?;
        }
    }

    Ok((root_window, root_visual, screen_width, screen_height))
}

fn create_window(
    stream: &mut TcpStream,
    window_id: u32,
    parent: u32,
    visual: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut req = Vec::new();
    req.push(1); // CreateWindow opcode
    req.push(24); // Depth (24-bit color)
    req.extend_from_slice(&(8u16 + 1).to_le_bytes()); // Request length in 4-byte units
    req.extend_from_slice(&window_id.to_le_bytes());
    req.extend_from_slice(&parent.to_le_bytes());
    req.extend_from_slice(&0i16.to_le_bytes()); // x
    req.extend_from_slice(&0i16.to_le_bytes()); // y
    req.extend_from_slice(&WINDOW_WIDTH.to_le_bytes());
    req.extend_from_slice(&WINDOW_HEIGHT.to_le_bytes());
    req.extend_from_slice(&0u16.to_le_bytes()); // Border width
    req.extend_from_slice(&1u16.to_le_bytes()); // Window class (InputOutput)
    req.extend_from_slice(&visual.to_le_bytes());
    req.extend_from_slice(&0x00000002u32.to_le_bytes()); // Value mask (background pixel)
    req.extend_from_slice(&0xFFFFFFu32.to_le_bytes()); // Background pixel (white)

    stream.write_all(&req)?;
    stream.flush()?;
    Ok(())
}

fn map_window(stream: &mut TcpStream, window_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    let mut req = Vec::new();
    req.push(8); // MapWindow opcode
    req.push(0); // Padding
    req.extend_from_slice(&2u16.to_le_bytes()); // Length
    req.extend_from_slice(&window_id.to_le_bytes());

    stream.write_all(&req)?;
    stream.flush()?;
    Ok(())
}

fn create_gc(
    stream: &mut TcpStream,
    gc_id: u32,
    drawable: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut req = Vec::new();
    req.push(55); // CreateGC opcode
    req.push(0); // Padding
    req.extend_from_slice(&4u16.to_le_bytes()); // Length
    req.extend_from_slice(&gc_id.to_le_bytes());
    req.extend_from_slice(&drawable.to_le_bytes());
    req.extend_from_slice(&0u32.to_le_bytes()); // Value mask (none)

    stream.write_all(&req)?;
    stream.flush()?;
    Ok(())
}

fn draw_colored_rectangles(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
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
        // Change GC foreground
        let mut req = Vec::new();
        req.push(56); // ChangeGC opcode
        req.push(0);
        req.extend_from_slice(&4u16.to_le_bytes()); // Length
        req.extend_from_slice(&gc.to_le_bytes());
        req.extend_from_slice(&0x00000004u32.to_le_bytes()); // Value mask (foreground)
        req.extend_from_slice(&color.to_le_bytes());
        stream.write_all(&req)?;

        // Draw filled rectangle
        let x = (spacing + (rect_width + spacing) * i as u16) as i16;
        let y = 20i16;

        let mut req = Vec::new();
        req.push(70); // PolyFillRectangle opcode
        req.push(0);
        req.extend_from_slice(&5u16.to_le_bytes()); // Length
        req.extend_from_slice(&window.to_le_bytes());
        req.extend_from_slice(&gc.to_le_bytes());
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
        req.extend_from_slice(&rect_width.to_le_bytes());
        req.extend_from_slice(&rect_height.to_le_bytes());
        stream.write_all(&req)?;
    }

    stream.flush()?;
    Ok(())
}

/// Draw connected lines (PolyLine - opcode 65)
fn draw_lines(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to orange
    let mut req = Vec::new();
    req.push(56); // ChangeGC opcode
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes()); // foreground mask
    req.extend_from_slice(&0xFF8000u32.to_le_bytes()); // orange
    stream.write_all(&req)?;

    // Draw a zigzag line (5 points = 4 segments)
    let points: [(i16, i16); 5] = [(20, 150), (60, 200), (100, 150), (140, 200), (180, 150)];

    let mut req = Vec::new();
    req.push(65); // PolyLine opcode
    req.push(0); // coordinate mode = Origin
    let length = 3 + points.len() as u16; // header + drawable + gc + points
    req.extend_from_slice(&length.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    for (x, y) in points {
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
    }
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolyLine (orange zigzag)");
    Ok(())
}

/// Draw rectangle outlines (PolyRectangle - opcode 67)
fn draw_rectangle_outlines(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to purple
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x8000FFu32.to_le_bytes()); // purple
    stream.write_all(&req)?;

    // Draw 2 rectangle outlines
    let rects: [(i16, i16, u16, u16); 2] = [(200, 150, 60, 40), (270, 150, 60, 40)];

    let mut req = Vec::new();
    req.push(67); // PolyRectangle opcode
    req.push(0);
    let length = 3 + (rects.len() * 2) as u16; // each rect is 8 bytes = 2 units
    req.extend_from_slice(&length.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    for (x, y, w, h) in rects {
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
        req.extend_from_slice(&w.to_le_bytes());
        req.extend_from_slice(&h.to_le_bytes());
    }
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolyRectangle (purple outlines)");
    Ok(())
}

/// Draw arc outlines (PolyArc - opcode 68)
fn draw_arcs(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to dark green
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x008000u32.to_le_bytes()); // dark green
    stream.write_all(&req)?;

    // Draw an arc (semicircle)
    // Arc angles are in 1/64 degree units
    // angle1=0, angle2=180*64=11520 for top half
    let mut req = Vec::new();
    req.push(68); // PolyArc opcode
    req.push(0);
    req.extend_from_slice(&6u16.to_le_bytes()); // 3 + 1 arc (12 bytes = 3 units)
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    // Arc: x, y, width, height, angle1, angle2
    req.extend_from_slice(&350i16.to_le_bytes()); // x
    req.extend_from_slice(&150i16.to_le_bytes()); // y
    req.extend_from_slice(&60u16.to_le_bytes()); // width
    req.extend_from_slice(&60u16.to_le_bytes()); // height
    req.extend_from_slice(&0i16.to_le_bytes()); // angle1 (0 degrees)
    req.extend_from_slice(&(180 * 64i16).to_le_bytes()); // angle2 (180 degrees)
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolyArc (dark green semicircle)");
    Ok(())
}

/// Draw filled arcs (PolyFillArc - opcode 71)
fn draw_filled_arcs(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to teal
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x008080u32.to_le_bytes()); // teal
    stream.write_all(&req)?;

    // Draw a filled pie slice (90 degrees)
    let mut req = Vec::new();
    req.push(71); // PolyFillArc opcode
    req.push(0);
    req.extend_from_slice(&6u16.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&420i16.to_le_bytes()); // x
    req.extend_from_slice(&150i16.to_le_bytes()); // y
    req.extend_from_slice(&60u16.to_le_bytes()); // width
    req.extend_from_slice(&60u16.to_le_bytes()); // height
    req.extend_from_slice(&(45 * 64i16).to_le_bytes()); // angle1 (45 degrees)
    req.extend_from_slice(&(90 * 64i16).to_le_bytes()); // angle2 (90 degrees)
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolyFillArc (teal pie slice)");
    Ok(())
}

/// Draw filled polygon (FillPoly - opcode 69)
fn draw_polygon(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to brown
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x8B4513u32.to_le_bytes()); // saddle brown
    stream.write_all(&req)?;

    // Draw a triangle
    let points: [(i16, i16); 3] = [(520, 200), (550, 150), (580, 200)];

    let mut req = Vec::new();
    req.push(69); // FillPoly opcode
    req.push(0);
    // Length = 4 (header + drawable + gc + shape/mode) + points
    let length = 4 + points.len() as u16;
    req.extend_from_slice(&length.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.push(2); // shape = Convex
    req.push(0); // coordinate mode = Origin
    req.extend_from_slice(&[0, 0]); // padding
    for (x, y) in points {
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
    }
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew FillPoly (brown triangle)");
    Ok(())
}

/// Draw points (PolyPoint - opcode 64)
fn draw_points(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to black
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x000000u32.to_le_bytes()); // black
    stream.write_all(&req)?;

    // Draw a grid of points
    let mut points = Vec::new();
    for i in 0..10 {
        for j in 0..5 {
            points.push((600 + i * 5, 150 + j * 5));
        }
    }

    let mut req = Vec::new();
    req.push(64); // PolyPoint opcode
    req.push(0); // coordinate mode = Origin
    let length = 3 + points.len() as u16;
    req.extend_from_slice(&length.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    for (x, y) in points {
        req.extend_from_slice(&(x as i16).to_le_bytes());
        req.extend_from_slice(&(y as i16).to_le_bytes());
    }
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolyPoint (black dot grid)");
    Ok(())
}

/// Draw line segments (PolySegment - opcode 66)
fn draw_segments(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to dark red
    let mut req = Vec::new();
    req.push(56);
    req.push(0);
    req.extend_from_slice(&4u16.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00000004u32.to_le_bytes());
    req.extend_from_slice(&0x800000u32.to_le_bytes()); // dark red
    stream.write_all(&req)?;

    // Draw an X shape using 2 independent segments
    let segments: [(i16, i16, i16, i16); 2] = [
        (700, 150, 750, 200), // diagonal 1
        (700, 200, 750, 150), // diagonal 2
    ];

    let mut req = Vec::new();
    req.push(66); // PolySegment opcode
    req.push(0);
    let length = 3 + (segments.len() * 2) as u16; // each segment is 8 bytes = 2 units
    req.extend_from_slice(&length.to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    for (x1, y1, x2, y2) in segments {
        req.extend_from_slice(&x1.to_le_bytes());
        req.extend_from_slice(&y1.to_le_bytes());
        req.extend_from_slice(&x2.to_le_bytes());
        req.extend_from_slice(&y2.to_le_bytes());
    }
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew PolySegment (dark red X)");
    Ok(())
}

/// Open a font (OpenFont - opcode 45)
fn open_font(
    stream: &mut TcpStream,
    font_id: u32,
    font_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let name_bytes = font_name.as_bytes();
    let name_len = name_bytes.len();
    let name_pad = (4 - (name_len % 4)) % 4;

    let mut req = Vec::new();
    req.push(45); // OpenFont opcode
    req.push(0); // Padding
    let length = 3 + (name_len + name_pad) / 4;
    req.extend_from_slice(&(length as u16).to_le_bytes());
    req.extend_from_slice(&font_id.to_le_bytes());
    req.extend_from_slice(&(name_len as u16).to_le_bytes());
    req.extend_from_slice(&[0, 0]); // padding
    req.extend_from_slice(name_bytes);
    req.extend(std::iter::repeat_n(0u8, name_pad));

    stream.write_all(&req)?;
    stream.flush()?;
    println!("  Opened font '{}'", font_name);
    Ok(())
}

/// Draw text (ImageText8 - opcode 76)
fn draw_text_test(
    stream: &mut TcpStream,
    window: u32,
    gc: u32,
    font_id: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Change GC foreground to navy blue and set font
    let mut req = Vec::new();
    req.push(56); // ChangeGC opcode
    req.push(0);
    req.extend_from_slice(&5u16.to_le_bytes()); // Length (3 header + 2 values)
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&0x00004004u32.to_le_bytes()); // foreground + font mask
    req.extend_from_slice(&0x000080u32.to_le_bytes()); // navy blue
    req.extend_from_slice(&font_id.to_le_bytes()); // font
    stream.write_all(&req)?;

    // Draw "X11" text
    let text = b"X11";
    let text_len = text.len();
    let text_pad = (4 - ((16 + text_len) % 4)) % 4;

    let mut req = Vec::new();
    req.push(76); // ImageText8 opcode
    req.push(text_len as u8);
    let length = 4 + (text_len + text_pad) / 4;
    req.extend_from_slice(&(length as u16).to_le_bytes());
    req.extend_from_slice(&window.to_le_bytes());
    req.extend_from_slice(&gc.to_le_bytes());
    req.extend_from_slice(&20i16.to_le_bytes()); // x
    req.extend_from_slice(&280i16.to_le_bytes()); // y
    req.extend_from_slice(text);
    req.extend(std::iter::repeat_n(0u8, text_pad));
    stream.write_all(&req)?;

    stream.flush()?;
    println!("  Drew ImageText8 (navy blue 'X11')");
    Ok(())
}

/// Validate that the colored rectangles are actually visible in the screenshot.
/// This searches for each expected color somewhere in the image.
fn validate_rectangles(screenshot: &screenshot::Screenshot) -> bool {
    // Expected colors (RGB) - use wider tolerance for Display P3 color space
    let expected_colors: [(u8, u8, u8, &str); 6] = [
        (255, 0, 0, "Red"),
        (0, 255, 0, "Green"),
        (0, 0, 255, "Blue"),
        (255, 255, 0, "Yellow"),
        (255, 0, 255, "Magenta"),
        (0, 255, 255, "Cyan"),
    ];

    // Debug: sample some pixels from the screenshot to see actual color values
    eprintln!(
        "Debug: Sampling pixels from screenshot {}x{}",
        screenshot.width, screenshot.height
    );
    let sample_points = [
        (50, 100),
        (180, 100),
        (310, 100),
        (440, 100),
        (570, 100),
        (700, 100),
    ];
    for (x, y) in sample_points {
        if x < screenshot.width && y < screenshot.height {
            let idx = ((y * screenshot.width + x) * 4) as usize;
            if idx + 3 < screenshot.data.len() {
                eprintln!(
                    "  Pixel ({}, {}): R={} G={} B={} A={}",
                    x,
                    y,
                    screenshot.data[idx],
                    screenshot.data[idx + 1],
                    screenshot.data[idx + 2],
                    screenshot.data[idx + 3]
                );
            }
        }
    }

    let mut all_found = true;

    for (expected_r, expected_g, expected_b, name) in expected_colors.iter() {
        let mut found = false;
        let mut found_count = 0;

        // Search for this color in the screenshot
        // Use wide tolerance (70) to account for Display P3 color space differences
        // macOS Display P3 can shift G channel by up to 64 for saturated sRGB colors
        let tolerance = 70u8;

        for y in 0..screenshot.height {
            for x in 0..screenshot.width {
                let idx = ((y * screenshot.width + x) * 4) as usize;
                if idx + 2 >= screenshot.data.len() {
                    continue;
                }

                let r = screenshot.data[idx];
                let g = screenshot.data[idx + 1];
                let b = screenshot.data[idx + 2];

                if color_matches(r, *expected_r, tolerance)
                    && color_matches(g, *expected_g, tolerance)
                    && color_matches(b, *expected_b, tolerance)
                {
                    found_count += 1;
                    if found_count >= 100 {
                        // Need at least 100 pixels of this color
                        found = true;
                        break;
                    }
                }
            }
            if found {
                break;
            }
        }

        if found {
            println!("  {} rectangle: FOUND ({} pixels)", name, found_count);
        } else {
            eprintln!(
                "  {} rectangle: NOT FOUND (only {} pixels)",
                name, found_count
            );
            all_found = false;
        }
    }

    all_found
}

fn color_matches(actual: u8, expected: u8, tolerance: u8) -> bool {
    actual.abs_diff(expected) <= tolerance
}
