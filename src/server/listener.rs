//! Server listener and connection handling
use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use super::Server;
use crate::protocol::setup::{SetupRequest, SetupResponse};

/// Start TCP listener for X11 connections
pub fn start_tcp_listener(
    display: u16,
    server: Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let addr = format!("0.0.0.0:{}", 6000 + display);
    let listener = TcpListener::bind(&addr)?;
    log::info!("Listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server = Arc::clone(&server);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, server) {
                        log::error!("Client error: {}", e);
                    }
                });
            }
            Err(e) => {
                log::error!("Connection failed: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(
    mut stream: TcpStream,
    server: Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("New client connection from {:?}", stream.peer_addr());

    // Read connection setup
    let setup_request = SetupRequest::parse(&mut stream)?;
    log::debug!("Setup request: {:?}", setup_request);

    // Send setup response
    let setup_response = {
        let server = server.lock().unwrap();
        create_setup_response(&server)
    };

    send_setup_response(&mut stream, &setup_response)?;
    log::info!("Client connected successfully");

    // Register client
    let client_id = {
        let mut server = server.lock().unwrap();
        server.register_client()
    };

    // Handle requests in a loop
    loop {
        // Read request header (4 bytes minimum)
        let mut header = [0u8; 4];
        match stream.read_exact(&mut header) {
            Ok(_) => {}
            Err(_) => {
                log::info!("Client {} disconnected", client_id);
                break;
            }
        }

        let opcode = header[0];
        let length = u16::from_le_bytes([header[2], header[3]]) as usize * 4;

        log::debug!("Received opcode {} (length {})", opcode, length);

        // Read rest of request
        let mut request_data = vec![0u8; length.saturating_sub(4)];
        if !request_data.is_empty() {
            stream.read_exact(&mut request_data)?;
        }

        // Handle basic opcodes needed for visual test
        match opcode {
            1 => handle_create_window(&mut stream, &header, &request_data, &server)?,
            4 => handle_destroy_window(&mut stream, &header, &request_data, &server)?,
            8 => handle_map_window(&mut stream, &header, &request_data, &server)?,
            10 => handle_unmap_window(&mut stream, &header, &request_data, &server)?,
            12 => handle_configure_window(&mut stream, &header, &request_data, &server)?,
            45 => handle_open_font(&mut stream, &header, &request_data, &server)?,
            46 => handle_close_font(&mut stream, &header, &request_data, &server)?,
            47 => handle_query_font(&mut stream, &header, &request_data, &server)?,
            49 => handle_list_fonts(&mut stream, &header, &request_data, &server)?,
            55 => handle_create_gc(&mut stream, &header, &request_data, &server)?,
            84 => handle_alloc_color(&mut stream, &header, &request_data, &server)?,
            85 => handle_alloc_named_color(&mut stream, &header, &request_data, &server)?,
            56 => handle_change_gc(&mut stream, &header, &request_data, &server)?,
            64 => handle_poly_point(&mut stream, &header, &request_data, &server)?,
            65 => handle_poly_line(&mut stream, &header, &request_data, &server)?,
            66 => handle_poly_segment(&mut stream, &header, &request_data, &server)?,
            67 => handle_poly_rectangle(&mut stream, &header, &request_data, &server)?,
            68 => handle_poly_arc(&mut stream, &header, &request_data, &server)?,
            69 => handle_fill_poly(&mut stream, &header, &request_data, &server)?,
            70 => handle_poly_fill_rectangle(&mut stream, &header, &request_data, &server)?,
            71 => handle_poly_fill_arc(&mut stream, &header, &request_data, &server)?,
            72 => handle_put_image(&mut stream, &header, &request_data, &server)?,
            74 => handle_poly_text8(&mut stream, &header, &request_data, &server)?,
            75 => handle_poly_text16(&mut stream, &header, &request_data, &server)?,
            76 => handle_image_text8(&mut stream, &header, &request_data, &server)?,
            77 => handle_image_text16(&mut stream, &header, &request_data, &server)?,
            _ => {
                log::debug!("Unhandled opcode: {}", opcode);
            }
        }
    }

    // Cleanup client
    {
        let mut server = server.lock().unwrap();
        server.handle_client_disconnect(client_id);
    }

    Ok(())
}

fn create_setup_response(server: &Server) -> SetupResponse {
    use crate::protocol::*;

    // Get screen info from the backend
    let screen_info = server.get_screen_info();

    SetupResponse::Success(SetupSuccess {
        protocol_major_version: 11,
        protocol_minor_version: 0,
        release_number: 1,
        resource_id_base: 0x02000000,
        resource_id_mask: 0x001FFFFF,
        motion_buffer_size: 256,
        maximum_request_length: 65535,
        image_byte_order: ByteOrder::LSBFirst,
        bitmap_format_bit_order: ByteOrder::LSBFirst,
        bitmap_format_scanline_unit: 32,
        bitmap_format_scanline_pad: 32,
        min_keycode: 8,
        max_keycode: 255,
        vendor: "X11Anywhere".to_string(),
        pixmap_formats: vec![Format {
            depth: 24,
            bits_per_pixel: 32,
            scanline_pad: 32,
        }],
        roots: vec![Screen {
            root: server.root_window(),
            default_colormap: Colormap::new(0x20),
            white_pixel: screen_info.white_pixel,
            black_pixel: screen_info.black_pixel,
            current_input_masks: 0,
            width_in_pixels: screen_info.width,
            height_in_pixels: screen_info.height,
            width_in_millimeters: screen_info.width_mm,
            height_in_millimeters: screen_info.height_mm,
            min_installed_maps: 1,
            max_installed_maps: 1,
            root_visual: screen_info.root_visual,
            backing_stores: 0,
            save_unders: false,
            root_depth: screen_info.root_depth,
            allowed_depths: vec![Depth {
                depth: screen_info.root_depth,
                visuals: vec![VisualType {
                    visual_id: screen_info.root_visual,
                    class: 4, // TrueColor
                    bits_per_rgb_value: 8,
                    colormap_entries: 256,
                    red_mask: 0xFF0000,
                    green_mask: 0x00FF00,
                    blue_mask: 0x0000FF,
                }],
            }],
        }],
    })
}

fn send_setup_response(
    stream: &mut TcpStream,
    response: &SetupResponse,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::protocol::ByteOrder;

    response.encode(stream, ByteOrder::LSBFirst)?;
    stream.flush()?;
    Ok(())
}

// Request handlers with actual implementation
fn handle_create_window(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::protocol::WindowClass;

    // Parse CreateWindow request
    // Format: depth(1), wid(4), parent(4), x(2), y(2), width(2), height(2), border_width(2), class(2), visual(4), value-mask(4), value-list(...)
    if data.len() < 24 {
        log::warn!("CreateWindow request too short");
        return Ok(());
    }

    let _depth = header[1];
    let wid = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let parent = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let x = i16::from_le_bytes([data[8], data[9]]);
    let y = i16::from_le_bytes([data[10], data[11]]);
    let width = u16::from_le_bytes([data[12], data[13]]);
    let height = u16::from_le_bytes([data[14], data[15]]);
    let border_width = u16::from_le_bytes([data[16], data[17]]);
    let class = u16::from_le_bytes([data[18], data[19]]);
    let visual = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let value_mask = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

    log::debug!(
        "CreateWindow: wid=0x{:x}, parent=0x{:x}, {}x{} at ({},{})",
        wid,
        parent,
        width,
        height,
        x,
        y
    );

    // Parse value list
    let mut background_pixel = None;
    let mut event_mask = 0u32;
    let mut offset = 28;

    // Background pixel (bit 1)
    if value_mask & 0x00000002 != 0 && offset + 4 <= data.len() {
        background_pixel = Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
    }

    // Event mask (bit 11)
    if value_mask & 0x00000800 != 0 && offset + 4 <= data.len() {
        event_mask = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
    }

    let window_class = match class {
        0 => WindowClass::CopyFromParent,
        1 => WindowClass::InputOutput,
        2 => WindowClass::InputOnly,
        _ => WindowClass::InputOutput,
    };

    let mut server = server.lock().unwrap();
    server.create_window(
        crate::protocol::Window::new(wid),
        crate::protocol::Window::new(parent),
        x,
        y,
        width,
        height,
        border_width,
        window_class,
        crate::protocol::VisualID::new(visual),
        background_pixel,
        event_mask,
    )?;

    Ok(())
}

fn handle_map_window(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse MapWindow request: window(4)
    if data.len() < 4 {
        log::warn!("MapWindow request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("MapWindow: window=0x{:x}", window);

    let mut server = server.lock().unwrap();
    server.map_window(crate::protocol::Window::new(window))?;

    Ok(())
}

fn handle_unmap_window(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse UnmapWindow request: window(4)
    if data.len() < 4 {
        log::warn!("UnmapWindow request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("UnmapWindow: window=0x{:x}", window);

    let mut server = server.lock().unwrap();
    server.unmap_window(crate::protocol::Window::new(window))?;

    Ok(())
}

fn handle_destroy_window(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse DestroyWindow request: window(4)
    if data.len() < 4 {
        log::warn!("DestroyWindow request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("DestroyWindow: window=0x{:x}", window);

    let mut server = server.lock().unwrap();
    server.destroy_window(crate::protocol::Window::new(window))?;

    Ok(())
}

fn handle_configure_window(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ConfigureWindow request: window(4), value-mask(2), pad(2), values(...)
    if data.len() < 4 {
        log::warn!("ConfigureWindow request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let _unused = header[1]; // mask is sometimes in header but we use data

    // ConfigureWindow has mask in data
    let value_mask = if data.len() >= 6 {
        u16::from_le_bytes([data[4], data[5]])
    } else {
        0
    };

    log::debug!(
        "ConfigureWindow: window=0x{:x}, mask=0x{:x}",
        window,
        value_mask
    );

    // Parse values based on mask
    let mut offset = 8; // Skip window(4), mask(2), pad(2)
    let mut x: Option<i16> = None;
    let mut y: Option<i16> = None;
    let mut width: Option<u16> = None;
    let mut height: Option<u16> = None;

    // Bit 0: x
    if value_mask & 0x0001 != 0 && offset + 4 <= data.len() {
        x = Some(i16::from_le_bytes([data[offset], data[offset + 1]]));
        offset += 4; // Values are padded to 4 bytes
    }
    // Bit 1: y
    if value_mask & 0x0002 != 0 && offset + 4 <= data.len() {
        y = Some(i16::from_le_bytes([data[offset], data[offset + 1]]));
        offset += 4;
    }
    // Bit 2: width
    if value_mask & 0x0004 != 0 && offset + 4 <= data.len() {
        width = Some(u16::from_le_bytes([data[offset], data[offset + 1]]));
        offset += 4;
    }
    // Bit 3: height
    if value_mask & 0x0008 != 0 && offset + 4 <= data.len() {
        height = Some(u16::from_le_bytes([data[offset], data[offset + 1]]));
        // offset += 4;
    }

    let mut server = server.lock().unwrap();
    server.configure_window(crate::protocol::Window::new(window), x, y, width, height)?;

    Ok(())
}

fn handle_create_gc(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse CreateGC request: cid(4), drawable(4), value-mask(4), value-list(...)
    if data.len() < 12 {
        log::warn!("CreateGC request too short");
        return Ok(());
    }

    let cid = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let drawable = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let value_mask = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    log::debug!(
        "CreateGC: cid=0x{:x}, drawable=0x{:x}, mask=0x{:x}",
        cid,
        drawable,
        value_mask
    );

    // Parse value list
    let mut foreground = None;
    let mut background = None;
    let mut offset = 12;

    // Function (bit 0)
    if value_mask & 0x00000001 != 0 {
        offset += 4;
    }

    // Plane mask (bit 1)
    if value_mask & 0x00000002 != 0 {
        offset += 4;
    }

    // Foreground (bit 2)
    if value_mask & 0x00000004 != 0 && offset + 4 <= data.len() {
        foreground = Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
    }

    // Background (bit 3)
    if value_mask & 0x00000008 != 0 && offset + 4 <= data.len() {
        background = Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
    }

    let mut server = server.lock().unwrap();
    server.create_gc(
        crate::protocol::GContext::new(cid),
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        foreground,
        background,
    )?;

    Ok(())
}

fn handle_change_gc(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ChangeGC request: gc(4), value-mask(4), value-list(...)
    if data.len() < 8 {
        log::warn!("ChangeGC request too short");
        return Ok(());
    }

    let gc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let value_mask = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    log::debug!("ChangeGC: gc=0x{:x}, mask=0x{:x}", gc, value_mask);

    // Parse value list
    let mut foreground = None;
    let mut background = None;
    let mut offset = 8;

    // Function (bit 0)
    if value_mask & 0x00000001 != 0 {
        offset += 4;
    }

    // Plane mask (bit 1)
    if value_mask & 0x00000002 != 0 {
        offset += 4;
    }

    // Foreground (bit 2)
    if value_mask & 0x00000004 != 0 && offset + 4 <= data.len() {
        foreground = Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
    }

    // Background (bit 3)
    if value_mask & 0x00000008 != 0 && offset + 4 <= data.len() {
        background = Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
    }

    if let Some(fg) = foreground {
        log::debug!(
            "ChangeGC: setting foreground=0x{:08x} (R={}, G={}, B={})",
            fg,
            (fg >> 16) & 0xff,
            (fg >> 8) & 0xff,
            fg & 0xff
        );
    }

    let mut server = server.lock().unwrap();
    server.change_gc(crate::protocol::GContext::new(gc), foreground, background)?;

    Ok(())
}

fn handle_poly_fill_rectangle(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyFillRectangle request: drawable(4), gc(4), rectangles(...)
    if data.len() < 8 {
        log::warn!("PolyFillRectangle request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse rectangles (each is 8 bytes: x, y, width, height)
    let mut rectangles = Vec::new();
    let mut offset = 8;
    while offset + 8 <= data.len() {
        let x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let width = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let height = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        rectangles.push(crate::protocol::Rectangle {
            x,
            y,
            width,
            height,
        });
        offset += 8;
    }

    log::debug!(
        "PolyFillRectangle: drawable=0x{:x}, gc=0x{:x}, {} rectangles",
        drawable,
        gc,
        rectangles.len()
    );

    let mut server = server.lock().unwrap();
    server.fill_rectangles(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &rectangles,
    )?;

    Ok(())
}

fn handle_poly_point(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyPoint request: drawable(4), gc(4), points(...)
    if data.len() < 8 {
        log::warn!("PolyPoint request too short");
        return Ok(());
    }

    let coordinate_mode = header[1]; // 0=Origin, 1=Previous
    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse points (each is 4 bytes: x, y)
    let mut points = Vec::new();
    let mut offset = 8;
    let mut prev_x = 0i16;
    let mut prev_y = 0i16;
    while offset + 4 <= data.len() {
        let mut x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let mut y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        if coordinate_mode == 1 && !points.is_empty() {
            x += prev_x;
            y += prev_y;
        }
        points.push(crate::protocol::Point { x, y });
        prev_x = x;
        prev_y = y;
        offset += 4;
    }

    log::debug!(
        "PolyPoint: drawable=0x{:x}, gc=0x{:x}, {} points",
        drawable,
        gc,
        points.len()
    );

    let mut server = server.lock().unwrap();
    server.draw_points(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &points,
    )?;

    Ok(())
}

fn handle_poly_line(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyLine request: drawable(4), gc(4), points(...)
    if data.len() < 8 {
        log::warn!("PolyLine request too short");
        return Ok(());
    }

    let coordinate_mode = header[1]; // 0=Origin, 1=Previous
    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse points (each is 4 bytes: x, y)
    let mut points = Vec::new();
    let mut offset = 8;
    let mut prev_x = 0i16;
    let mut prev_y = 0i16;
    while offset + 4 <= data.len() {
        let mut x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let mut y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        if coordinate_mode == 1 && !points.is_empty() {
            x += prev_x;
            y += prev_y;
        }
        points.push(crate::protocol::Point { x, y });
        prev_x = x;
        prev_y = y;
        offset += 4;
    }

    log::debug!(
        "PolyLine: drawable=0x{:x}, gc=0x{:x}, {} points",
        drawable,
        gc,
        points.len()
    );

    let mut server = server.lock().unwrap();
    server.draw_lines(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &points,
    )?;

    Ok(())
}

fn handle_poly_segment(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolySegment request: drawable(4), gc(4), segments(...)
    if data.len() < 8 {
        log::warn!("PolySegment request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse segments (each is 8 bytes: x1, y1, x2, y2)
    let mut segments = Vec::new();
    let mut offset = 8;
    while offset + 8 <= data.len() {
        let x1 = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let y1 = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let x2 = i16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let y2 = i16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        segments.push((x1, y1, x2, y2));
        offset += 8;
    }

    log::debug!(
        "PolySegment: drawable=0x{:x}, gc=0x{:x}, {} segments",
        drawable,
        gc,
        segments.len()
    );

    let mut server = server.lock().unwrap();
    server.draw_segments(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &segments,
    )?;

    Ok(())
}

fn handle_poly_rectangle(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyRectangle request: drawable(4), gc(4), rectangles(...)
    if data.len() < 8 {
        log::warn!("PolyRectangle request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse rectangles (each is 8 bytes: x, y, width, height)
    let mut rectangles = Vec::new();
    let mut offset = 8;
    while offset + 8 <= data.len() {
        let x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let width = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let height = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        rectangles.push(crate::protocol::Rectangle {
            x,
            y,
            width,
            height,
        });
        offset += 8;
    }

    log::debug!(
        "PolyRectangle: drawable=0x{:x}, gc=0x{:x}, {} rectangles",
        drawable,
        gc,
        rectangles.len()
    );

    let mut server = server.lock().unwrap();
    server.draw_rectangles(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &rectangles,
    )?;

    Ok(())
}

fn handle_poly_arc(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyArc request: drawable(4), gc(4), arcs(...)
    if data.len() < 8 {
        log::warn!("PolyArc request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse arcs (each is 12 bytes: x, y, width, height, angle1, angle2)
    let mut arcs = Vec::new();
    let mut offset = 8;
    while offset + 12 <= data.len() {
        let x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let width = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let height = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        let angle1 = i16::from_le_bytes([data[offset + 8], data[offset + 9]]);
        let angle2 = i16::from_le_bytes([data[offset + 10], data[offset + 11]]);
        arcs.push(crate::protocol::Arc {
            x,
            y,
            width,
            height,
            angle1,
            angle2,
        });
        offset += 12;
    }

    log::debug!(
        "PolyArc: drawable=0x{:x}, gc=0x{:x}, {} arcs",
        drawable,
        gc,
        arcs.len()
    );

    let mut server = server.lock().unwrap();
    server.draw_arcs(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &arcs,
    )?;

    Ok(())
}

fn handle_fill_poly(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse FillPoly request: drawable(4), gc(4), shape(1), coordinate-mode(1), pad(2), points(...)
    if data.len() < 12 {
        log::warn!("FillPoly request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let _shape = data[8]; // 0=Complex, 1=Nonconvex, 2=Convex
    let coordinate_mode = data[9]; // 0=Origin, 1=Previous

    // Parse points (each is 4 bytes: x, y)
    let mut points = Vec::new();
    let mut offset = 12;
    let mut prev_x = 0i16;
    let mut prev_y = 0i16;
    while offset + 4 <= data.len() {
        let mut x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let mut y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        if coordinate_mode == 1 && !points.is_empty() {
            x += prev_x;
            y += prev_y;
        }
        points.push(crate::protocol::Point { x, y });
        prev_x = x;
        prev_y = y;
        offset += 4;
    }

    log::debug!(
        "FillPoly: drawable=0x{:x}, gc=0x{:x}, {} points",
        drawable,
        gc,
        points.len()
    );

    let _ = header; // suppress unused warning

    let mut server = server.lock().unwrap();
    server.fill_polygon(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &points,
    )?;

    Ok(())
}

fn handle_poly_fill_arc(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyFillArc request: drawable(4), gc(4), arcs(...)
    if data.len() < 8 {
        log::warn!("PolyFillArc request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Parse arcs (each is 12 bytes: x, y, width, height, angle1, angle2)
    let mut arcs = Vec::new();
    let mut offset = 8;
    while offset + 12 <= data.len() {
        let x = i16::from_le_bytes([data[offset], data[offset + 1]]);
        let y = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
        let width = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
        let height = u16::from_le_bytes([data[offset + 6], data[offset + 7]]);
        let angle1 = i16::from_le_bytes([data[offset + 8], data[offset + 9]]);
        let angle2 = i16::from_le_bytes([data[offset + 10], data[offset + 11]]);
        arcs.push(crate::protocol::Arc {
            x,
            y,
            width,
            height,
            angle1,
            angle2,
        });
        offset += 12;
    }

    log::debug!(
        "PolyFillArc: drawable=0x{:x}, gc=0x{:x}, {} arcs",
        drawable,
        gc,
        arcs.len()
    );

    let mut server = server.lock().unwrap();
    server.fill_arcs(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &arcs,
    )?;

    Ok(())
}

fn handle_put_image(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PutImage request: drawable(4), gc(4), width(2), height(2), dst-x(2), dst-y(2),
    //                         left-pad(1), depth(1), pad(2), data(...)
    if data.len() < 16 {
        log::warn!("PutImage request too short");
        return Ok(());
    }

    let format = header[1]; // 0=Bitmap, 1=XYPixmap, 2=ZPixmap
    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let width = u16::from_le_bytes([data[8], data[9]]);
    let height = u16::from_le_bytes([data[10], data[11]]);
    let dst_x = i16::from_le_bytes([data[12], data[13]]);
    let dst_y = i16::from_le_bytes([data[14], data[15]]);
    let _left_pad = data[16];
    let depth = data[17];
    // data[18..20] is padding
    let image_data = &data[20..];

    log::debug!(
        "PutImage: drawable=0x{:x}, gc=0x{:x}, {}x{} at ({},{}), format={}, depth={}, {} bytes",
        drawable,
        gc,
        width,
        height,
        dst_x,
        dst_y,
        format,
        depth,
        image_data.len()
    );

    let mut server = server.lock().unwrap();
    server.put_image(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        width,
        height,
        dst_x,
        dst_y,
        depth,
        format,
        image_data,
    )?;

    Ok(())
}

fn handle_open_font(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse OpenFont request: fid(4), name_length(2), pad(2), name
    if data.len() < 4 {
        log::warn!("OpenFont request too short");
        return Ok(());
    }

    let font_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let name_length = u16::from_le_bytes([data[4], data[5]]) as usize;
    let name_end = (8 + name_length).min(data.len());
    let font_name = String::from_utf8_lossy(&data[8..name_end]).to_string();

    log::debug!("OpenFont: id=0x{:x}, name={:?}", font_id, font_name);

    // Store the font reference in the server
    let mut server = server.lock().unwrap();
    server.open_font(font_id, &font_name);

    Ok(())
}

fn handle_close_font(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse CloseFont request: fid(4)
    if data.len() < 4 {
        log::warn!("CloseFont request too short");
        return Ok(());
    }

    let font_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("CloseFont: id=0x{:x}", font_id);

    let mut server = server.lock().unwrap();
    server.close_font(font_id);

    Ok(())
}

fn handle_query_font(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse QueryFont request: font(4)
    if data.len() < 4 {
        log::warn!("QueryFont request too short");
        return Ok(());
    }

    let font_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("QueryFont: font=0x{:x}", font_id);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Get font info
    let (ascent, descent, char_width) = if let Some(font_info) = server.query_font(font_id) {
        (font_info.ascent, font_info.descent, font_info.char_width)
    } else {
        // Return default values if font not found
        log::warn!("QueryFont: font 0x{:x} not found, using defaults", font_id);
        (12, 4, 8)
    };

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply =
        encoder.encode_query_font_reply(sequence, ascent, descent, char_width, ascent + descent);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_list_fonts(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ListFonts request: max_names(2), pattern_length(2), pattern(n)
    if data.len() < 4 {
        log::warn!("ListFonts request too short");
        return Ok(());
    }

    let max_names = u16::from_le_bytes([data[0], data[1]]);
    let pattern_length = u16::from_le_bytes([data[2], data[3]]) as usize;
    let pattern_end = (4 + pattern_length).min(data.len());
    let pattern = String::from_utf8_lossy(&data[4..pattern_end]).to_string();

    log::debug!("ListFonts: max_names={}, pattern={:?}", max_names, pattern);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let font_names = server.list_fonts(&pattern, max_names);

    log::debug!("ListFonts: returning {} fonts", font_names.len());

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_list_fonts_reply(sequence, &font_names);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_alloc_color(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse AllocColor request: colormap(4), red(2), green(2), blue(2), pad(2)
    if data.len() < 10 {
        log::warn!("AllocColor request too short");
        return Ok(());
    }

    let _colormap = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let red = u16::from_le_bytes([data[4], data[5]]);
    let green = u16::from_le_bytes([data[6], data[7]]);
    let blue = u16::from_le_bytes([data[8], data[9]]);

    log::debug!("AllocColor: red={}, green={}, blue={}", red, green, blue);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let pixel = server.alloc_color(red, green, blue);

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_alloc_color_reply(sequence, pixel, red, green, blue);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_alloc_named_color(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse AllocNamedColor request: colormap(4), name_length(2), pad(2), name(n)
    if data.len() < 4 {
        log::warn!("AllocNamedColor request too short");
        return Ok(());
    }

    let _colormap = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let name_length = u16::from_le_bytes([data[4], data[5]]) as usize;
    let name_end = (8 + name_length).min(data.len());
    let name = String::from_utf8_lossy(&data[8..name_end]).to_string();

    log::debug!("AllocNamedColor: name={:?}", name);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Look up the named color
    let (pixel, exact_red, exact_green, exact_blue, visual_red, visual_green, visual_blue) =
        if let Some(color_info) = server.lookup_named_color(&name) {
            color_info
        } else {
            // Return black for unknown color names
            log::warn!("AllocNamedColor: unknown color '{}'", name);
            (0, 0, 0, 0, 0, 0, 0)
        };

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_alloc_named_color_reply(
        sequence,
        pixel,
        exact_red,
        exact_green,
        exact_blue,
        visual_red,
        visual_green,
        visual_blue,
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_image_text8(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ImageText8 request: drawable(4), gc(4), x(2), y(2), string(n)
    if data.len() < 12 {
        log::warn!("ImageText8 request too short");
        return Ok(());
    }

    let string_len = header[1] as usize;
    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let x = i16::from_le_bytes([data[8], data[9]]);
    let y = i16::from_le_bytes([data[10], data[11]]);

    let text_end = (12 + string_len).min(data.len());
    let text = String::from_utf8_lossy(&data[12..text_end]).to_string();

    log::debug!(
        "ImageText8: drawable=0x{:x}, gc=0x{:x}, ({},{}), text={:?}",
        drawable,
        gc,
        x,
        y,
        text
    );

    let mut server = server.lock().unwrap();
    server.draw_text(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        x,
        y,
        &text,
    )?;

    Ok(())
}

fn handle_image_text16(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ImageText16 request: drawable(4), gc(4), x(2), y(2), chars(n*2)
    if data.len() < 12 {
        log::warn!("ImageText16 request too short");
        return Ok(());
    }

    let string_len = header[1] as usize;
    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let x = i16::from_le_bytes([data[8], data[9]]);
    let y = i16::from_le_bytes([data[10], data[11]]);

    // Parse 16-bit characters (CHAR2B format: byte1=high, byte2=low)
    let char_data = &data[12..];
    let mut text = String::new();
    for i in (0..string_len * 2).step_by(2) {
        if i + 1 < char_data.len() {
            let high = char_data[i] as u16;
            let low = char_data[i + 1] as u16;
            let codepoint = (high << 8) | low;
            if let Some(ch) = char::from_u32(codepoint as u32) {
                text.push(ch);
            }
        }
    }

    log::debug!(
        "ImageText16: drawable=0x{:x}, gc=0x{:x}, ({},{}), text={:?}",
        drawable,
        gc,
        x,
        y,
        text
    );

    let mut server = server.lock().unwrap();
    server.draw_text(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        x,
        y,
        &text,
    )?;

    Ok(())
}

fn handle_poly_text8(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyText8 request: drawable(4), gc(4), x(2), y(2), items(...)
    if data.len() < 12 {
        log::warn!("PolyText8 request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let mut x = i16::from_le_bytes([data[8], data[9]]);
    let y = i16::from_le_bytes([data[10], data[11]]);

    // Parse text items (each item: len(1), delta(1), string(len))
    // If len=255, it's a font-shift item (4 bytes total: 255, font(3))
    let mut offset = 12;
    let mut server = server.lock().unwrap();

    while offset < data.len() {
        let len = data[offset] as usize;
        if len == 0 {
            break; // End of items
        }
        if len == 255 {
            // Font-shift item - skip for now
            offset += 4;
            continue;
        }
        if offset + 2 + len > data.len() {
            break;
        }

        let delta = data[offset + 1] as i16;
        x += delta;

        let text = String::from_utf8_lossy(&data[offset + 2..offset + 2 + len]).to_string();

        log::debug!(
            "PolyText8 item: drawable=0x{:x}, gc=0x{:x}, ({},{}), text={:?}",
            drawable,
            gc,
            x,
            y,
            text
        );

        server.draw_text(
            crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
            crate::protocol::GContext::new(gc),
            x,
            y,
            &text,
        )?;

        // Advance x by approximate text width (crude estimate)
        x += (len * 8) as i16;
        offset += 2 + len;
    }

    Ok(())
}

fn handle_poly_text16(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse PolyText16 request: drawable(4), gc(4), x(2), y(2), items(...)
    if data.len() < 12 {
        log::warn!("PolyText16 request too short");
        return Ok(());
    }

    let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let mut x = i16::from_le_bytes([data[8], data[9]]);
    let y = i16::from_le_bytes([data[10], data[11]]);

    // Parse text items (each item: len(1), delta(1), chars(len*2))
    let mut offset = 12;
    let mut server = server.lock().unwrap();

    while offset < data.len() {
        let len = data[offset] as usize;
        if len == 0 {
            break; // End of items
        }
        if len == 255 {
            // Font-shift item - skip for now
            offset += 4;
            continue;
        }
        if offset + 2 + len * 2 > data.len() {
            break;
        }

        let delta = data[offset + 1] as i16;
        x += delta;

        // Parse 16-bit characters
        let char_data = &data[offset + 2..offset + 2 + len * 2];
        let mut text = String::new();
        for i in (0..len * 2).step_by(2) {
            if i + 1 < char_data.len() {
                let high = char_data[i] as u16;
                let low = char_data[i + 1] as u16;
                let codepoint = (high << 8) | low;
                if let Some(ch) = char::from_u32(codepoint as u32) {
                    text.push(ch);
                }
            }
        }

        log::debug!(
            "PolyText16 item: drawable=0x{:x}, gc=0x{:x}, ({},{}), text={:?}",
            drawable,
            gc,
            x,
            y,
            text
        );

        server.draw_text(
            crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
            crate::protocol::GContext::new(gc),
            x,
            y,
            &text,
        )?;

        // Advance x by approximate text width
        x += (len * 8) as i16;
        offset += 2 + len * 2;
    }

    Ok(())
}
