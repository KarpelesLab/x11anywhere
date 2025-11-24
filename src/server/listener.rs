///! Server listener and connection handling

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

        // Read rest of request
        let mut request_data = vec![0u8; length.saturating_sub(4)];
        if request_data.len() > 0 {
            stream.read_exact(&mut request_data)?;
        }

        // Handle basic opcodes needed for visual test
        match opcode {
            1 => handle_create_window(&mut stream, &header, &request_data, &server)?,
            8 => handle_map_window(&mut stream, &header, &request_data, &server)?,
            55 => handle_create_gc(&mut stream, &header, &request_data, &server)?,
            56 => handle_change_gc(&mut stream, &header, &request_data, &server)?,
            70 => handle_poly_fill_rectangle(&mut stream, &header, &request_data, &server)?,
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
            white_pixel: 0xFFFFFF,
            black_pixel: 0x000000,
            current_input_masks: 0,
            width_in_pixels: 1920,
            height_in_pixels: 1080,
            width_in_millimeters: 508,
            height_in_millimeters: 285,
            min_installed_maps: 1,
            max_installed_maps: 1,
            root_visual: VisualID::new(0x21),
            backing_stores: 0,
            save_unders: false,
            root_depth: 24,
            allowed_depths: vec![Depth {
                depth: 24,
                visuals: vec![VisualType {
                    visual_id: VisualID::new(0x21),
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

    log::debug!("CreateWindow: wid=0x{:x}, parent=0x{:x}, {}x{} at ({},{})", wid, parent, width, height, x, y);

    // Parse value list
    let mut background_pixel = None;
    let mut event_mask = 0u32;
    let mut offset = 28;

    // Background pixel (bit 1)
    if value_mask & 0x00000002 != 0 && offset + 4 <= data.len() {
        background_pixel = Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]));
        offset += 4;
    }

    // Event mask (bit 11)
    if value_mask & 0x00000800 != 0 && offset + 4 <= data.len() {
        event_mask = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
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

    log::debug!("CreateGC: cid=0x{:x}, drawable=0x{:x}, mask=0x{:x}", cid, drawable, value_mask);

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
        foreground = Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]));
        offset += 4;
    }

    // Background (bit 3)
    if value_mask & 0x00000008 != 0 && offset + 4 <= data.len() {
        background = Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]));
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
        foreground = Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]));
        offset += 4;
    }

    // Background (bit 3)
    if value_mask & 0x00000008 != 0 && offset + 4 <= data.len() {
        background = Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]));
    }

    let mut server = server.lock().unwrap();
    server.change_gc(
        crate::protocol::GContext::new(gc),
        foreground,
        background,
    )?;

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
        let x = i16::from_le_bytes([data[offset], data[offset+1]]);
        let y = i16::from_le_bytes([data[offset+2], data[offset+3]]);
        let width = u16::from_le_bytes([data[offset+4], data[offset+5]]);
        let height = u16::from_le_bytes([data[offset+6], data[offset+7]]);
        rectangles.push(crate::protocol::Rectangle { x, y, width, height });
        offset += 8;
    }

    log::debug!("PolyFillRectangle: drawable=0x{:x}, gc=0x{:x}, {} rectangles", drawable, gc, rectangles.len());

    let mut server = server.lock().unwrap();
    server.fill_rectangles(
        crate::protocol::Drawable::Window(crate::protocol::Window::new(drawable)),
        crate::protocol::GContext::new(gc),
        &rectangles,
    )?;

    Ok(())
}
