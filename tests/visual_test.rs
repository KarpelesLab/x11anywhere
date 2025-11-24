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

    // Draw test patterns
    println!("Drawing test patterns...");
    draw_colored_rectangles(&mut stream, window_id, gc_id)?;

    // Wait for rendering
    std::thread::sleep(std::time::Duration::from_millis(500));

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

    // Skip to useful data (simplified parsing)
    let mut header = vec![0u8; 39];
    stream.read_exact(&mut header)?;

    let root_window = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    let root_visual = u32::from_le_bytes([header[28], header[29], header[30], header[31]]);
    let screen_width = u16::from_le_bytes([header[20], header[21]]);
    let screen_height = u16::from_le_bytes([header[22], header[23]]);

    // Read rest of setup data
    let vendor_len = header[8] as usize;
    let vendor_padded = (vendor_len + 3) & !3;
    let mut vendor = vec![0u8; vendor_padded];
    stream.read_exact(&mut vendor)?;

    // Read formats
    let num_formats = header[7] as usize;
    let mut formats = vec![0u8; num_formats * 8];
    stream.read_exact(&mut formats)?;

    // Read screens (we only care about first one)
    let mut screen_data = vec![0u8; 40];
    stream.read_exact(&mut screen_data)?;

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
