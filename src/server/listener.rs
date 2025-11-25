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

        // Handle X11 protocol requests
        match opcode {
            1 => handle_create_window(&mut stream, &header, &request_data, &server)?,
            3 => handle_get_window_attributes(&mut stream, &header, &request_data, &server)?,
            4 => handle_destroy_window(&mut stream, &header, &request_data, &server)?,
            8 => handle_map_window(&mut stream, &header, &request_data, &server)?,
            10 => handle_unmap_window(&mut stream, &header, &request_data, &server)?,
            12 => handle_configure_window(&mut stream, &header, &request_data, &server)?,
            14 => handle_get_geometry(&mut stream, &header, &request_data, &server)?,
            15 => handle_query_tree(&mut stream, &header, &request_data, &server)?,
            16 => handle_intern_atom(&mut stream, &header, &request_data, &server)?,
            17 => handle_get_atom_name(&mut stream, &header, &request_data, &server)?,
            18 => handle_change_property(&mut stream, &header, &request_data, &server)?,
            19 => handle_delete_property(&mut stream, &header, &request_data, &server)?,
            20 => handle_get_property(&mut stream, &header, &request_data, &server)?,
            21 => handle_list_properties(&mut stream, &header, &request_data, &server)?,
            22 => handle_set_selection_owner(&mut stream, &header, &request_data, &server)?,
            23 => handle_get_selection_owner(&mut stream, &header, &request_data, &server)?,
            28 => handle_grab_server(&mut stream, &header, &request_data, &server)?,
            29 => handle_ungrab_server(&mut stream, &header, &request_data, &server)?,
            38 => handle_query_pointer(&mut stream, &header, &request_data, &server)?,
            40 => handle_translate_coordinates(&mut stream, &header, &request_data, &server)?,
            42 => handle_set_input_focus(&mut stream, &header, &request_data, &server)?,
            43 => handle_get_input_focus(&mut stream, &header, &request_data, &server)?,
            44 => handle_query_keymap(&mut stream, &header, &request_data, &server)?,
            45 => handle_open_font(&mut stream, &header, &request_data, &server)?,
            46 => handle_close_font(&mut stream, &header, &request_data, &server)?,
            47 => handle_query_font(&mut stream, &header, &request_data, &server)?,
            49 => handle_list_fonts(&mut stream, &header, &request_data, &server)?,
            53 => handle_create_pixmap(&mut stream, &header, &request_data, &server)?,
            54 => handle_free_pixmap(&mut stream, &header, &request_data, &server)?,
            55 => handle_create_gc(&mut stream, &header, &request_data, &server)?,
            56 => handle_change_gc(&mut stream, &header, &request_data, &server)?,
            57 => handle_free_gc(&mut stream, &header, &request_data, &server)?,
            60 => handle_clear_area(&mut stream, &header, &request_data, &server)?,
            84 => handle_alloc_color(&mut stream, &header, &request_data, &server)?,
            85 => handle_alloc_named_color(&mut stream, &header, &request_data, &server)?,
            98 => handle_query_extension(&mut stream, &header, &request_data, &server)?,
            99 => handle_list_extensions(&mut stream, &header, &request_data, &server)?,
            104 => handle_bell(&mut stream, &header, &request_data, &server)?,
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
            // Extension opcodes (129+)
            129..=255 => {
                super::extensions::handle_extension_request(
                    &mut stream,
                    &header,
                    &request_data,
                    opcode,
                )?;
            }
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

fn handle_query_extension(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse QueryExtension request: name_length(2), pad(2), name(n)
    if data.len() < 4 {
        log::warn!("QueryExtension request too short");
        return Ok(());
    }

    let name_length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let name_end = (4 + name_length).min(data.len());
    let name = String::from_utf8_lossy(&data[4..name_end]).to_string();

    log::debug!("QueryExtension: name={:?}", name);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Look up the extension
    let (present, major_opcode, first_event, first_error) =
        if let Some(ext_info) = server.query_extension(&name) {
            (
                true,
                ext_info.major_opcode,
                ext_info.first_event,
                ext_info.first_error,
            )
        } else {
            log::debug!("QueryExtension: extension '{}' not found", name);
            (false, 0, 0, 0)
        };

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_query_extension_reply(
        sequence,
        present,
        major_opcode,
        first_event,
        first_error,
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_list_extensions(
    stream: &mut TcpStream,
    header: &[u8],
    _data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("ListExtensions");

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let extension_names = server.list_extensions();

    log::debug!(
        "ListExtensions: returning {} extensions",
        extension_names.len()
    );

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_list_extensions_reply(sequence, &extension_names);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_get_window_attributes(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse GetWindowAttributes request: window(4)
    if data.len() < 4 {
        log::warn!("GetWindowAttributes request too short");
        return Ok(());
    }

    let window_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("GetWindowAttributes: window=0x{:x}", window_id);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let _server = server.lock().unwrap();

    // Return default attributes for the window
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_get_window_attributes_reply(
        sequence,
        crate::protocol::types::VisualID::new(0x21), // Default visual
        crate::protocol::types::WindowClass::InputOutput,
        0, // bit_gravity: Forget
        crate::protocol::types::BackingStore::NotUseful,
        0xFFFFFFFF, // backing_planes
        0,          // backing_pixel
        false,      // save_under
        true,       // map_is_installed
        crate::protocol::types::MapState::Viewable,
        false,                                       // override_redirect
        crate::protocol::types::Colormap::new(0x20), // Default colormap
        0xFFFFFFFF,                                  // all_event_masks
        0xFFFFFFFF,                                  // your_event_mask
        0,                                           // do_not_propagate_mask
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_get_geometry(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse GetGeometry request: drawable(4)
    if data.len() < 4 {
        log::warn!("GetGeometry request too short");
        return Ok(());
    }

    let drawable_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("GetGeometry: drawable=0x{:x}", drawable_id);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Get window dimensions if it's a window we know about
    let (x, y, width, height) = if let Some(_backend_window) = server
        .windows
        .get(&crate::protocol::types::Window::new(drawable_id))
    {
        // Try to get actual dimensions from backend - for now return defaults
        (0i16, 0i16, 800u16, 600u16)
    } else {
        // Could be root window or unknown - return defaults
        (0i16, 0i16, 1920u16, 1080u16)
    };

    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_get_geometry_reply(
        sequence,
        24, // depth (24-bit TrueColor)
        server.root_window(),
        x,
        y,
        width,
        height,
        0, // border_width
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_query_tree(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse QueryTree request: window(4)
    if data.len() < 4 {
        log::warn!("QueryTree request too short");
        return Ok(());
    }

    let window_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("QueryTree: window=0x{:x}", window_id);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Get the root window
    let root = server.root_window();

    // For now, return empty children list with root as parent
    // TODO: Track window hierarchy properly
    let parent = if window_id == root.id().get() {
        crate::protocol::types::Window::NONE
    } else {
        root
    };

    // Get all windows as children of root for now
    let children: Vec<crate::protocol::types::Window> = if window_id == root.id().get() {
        server.windows.keys().cloned().collect()
    } else {
        Vec::new()
    };

    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_query_tree_reply(sequence, root, parent, &children);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_bell(
    _stream: &mut TcpStream,
    header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Bell request has percent in the data byte of header
    let percent = header[1] as i8;
    log::debug!("Bell: percent={}", percent);

    // Bell is a no-op for now (would need platform-specific audio)
    // Could use platform beep APIs in future:
    // - Windows: MessageBeep or Beep
    // - macOS: NSBeep
    // - Linux: XBell passthrough or console bell

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

fn handle_intern_atom(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse InternAtom request: only_if_exists(1 in header), name_length(2), pad(2), name(n)
    if data.len() < 4 {
        log::warn!("InternAtom request too short");
        return Ok(());
    }

    let only_if_exists = header[1] != 0;
    let name_length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let name_end = (4 + name_length).min(data.len());
    let name = String::from_utf8_lossy(&data[4..name_end]).to_string();

    log::debug!(
        "InternAtom: name={:?}, only_if_exists={}",
        name,
        only_if_exists
    );

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let mut server = server.lock().unwrap();
    let atom = server.intern_atom(&name, only_if_exists);

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply =
        encoder.encode_intern_atom_reply(sequence, atom.unwrap_or(crate::protocol::Atom::new(0)));

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_get_atom_name(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse GetAtomName request: atom(4)
    if data.len() < 4 {
        log::warn!("GetAtomName request too short");
        return Ok(());
    }

    let atom_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("GetAtomName: atom=0x{:x}", atom_id);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let name = server
        .get_atom_name(crate::protocol::Atom::new(atom_id))
        .unwrap_or("");

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_get_atom_name_reply(sequence, name);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_change_property(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ChangeProperty request: mode(1 in header), window(4), property(4), type(4), format(1), pad(3), data_length(4), data(n)
    if data.len() < 20 {
        log::warn!("ChangeProperty request too short");
        return Ok(());
    }

    let mode = header[1];
    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let property = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let type_ = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let format = data[12];
    let data_length = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;

    // Calculate actual byte length based on format
    let byte_length = match format {
        8 => data_length,
        16 => data_length * 2,
        32 => data_length * 4,
        _ => data_length,
    };

    let prop_data_end = (20 + byte_length).min(data.len());
    let prop_data = data[20..prop_data_end].to_vec();

    log::debug!(
        "ChangeProperty: window=0x{:x}, property=0x{:x}, type=0x{:x}, format={}, mode={}, {} bytes",
        window,
        property,
        type_,
        format,
        mode,
        prop_data.len()
    );

    let mut server = server.lock().unwrap();
    server.change_property(
        crate::protocol::Window::new(window),
        crate::protocol::Atom::new(property),
        crate::protocol::Atom::new(type_),
        format,
        mode,
        prop_data,
    );

    // No reply for ChangeProperty
    Ok(())
}

fn handle_delete_property(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse DeleteProperty request: window(4), property(4)
    if data.len() < 8 {
        log::warn!("DeleteProperty request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let property = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    log::debug!(
        "DeleteProperty: window=0x{:x}, property=0x{:x}",
        window,
        property
    );

    let mut server = server.lock().unwrap();
    server.delete_property(
        crate::protocol::Window::new(window),
        crate::protocol::Atom::new(property),
    );

    // No reply for DeleteProperty
    Ok(())
}

fn handle_get_property(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse GetProperty request: delete(1 in header), window(4), property(4), type(4), long_offset(4), long_length(4)
    if data.len() < 20 {
        log::warn!("GetProperty request too short");
        return Ok(());
    }

    let delete = header[1] != 0;
    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let property = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let type_ = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let long_offset = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let long_length = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

    log::debug!(
        "GetProperty: window=0x{:x}, property=0x{:x}, type=0x{:x}, delete={}",
        window,
        property,
        type_,
        delete
    );

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Get property value
    let (format, prop_type, value) = if let Some(prop) = server.get_property(
        crate::protocol::Window::new(window),
        crate::protocol::Atom::new(property),
        if type_ == 0 {
            None
        } else {
            Some(crate::protocol::Atom::new(type_))
        },
        long_offset,
        long_length,
        delete,
    ) {
        (prop.format, prop.type_, prop.data.clone())
    } else {
        (0, crate::protocol::Atom::new(0), Vec::new())
    };

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_get_property_reply(sequence, format, prop_type, 0, &value);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_list_properties(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ListProperties request: window(4)
    if data.len() < 4 {
        log::warn!("ListProperties request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("ListProperties: window=0x{:x}", window);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let properties = server.list_properties(crate::protocol::Window::new(window));

    // Encode reply: reply(1), pad(1), sequence(2), length(4), num_atoms(2), pad(22), atoms(n*4)
    let num_atoms = properties.len() as u16;
    let reply_length = (num_atoms as u32 + 5) / 2; // 4-byte units after header
    let mut reply = vec![0u8; 32 + properties.len() * 4];

    reply[0] = 1; // Reply
    reply[2..4].copy_from_slice(&sequence.to_le_bytes());
    reply[4..8].copy_from_slice(&reply_length.to_le_bytes());
    reply[8..10].copy_from_slice(&num_atoms.to_le_bytes());

    for (i, atom) in properties.iter().enumerate() {
        let offset = 32 + i * 4;
        reply[offset..offset + 4].copy_from_slice(&atom.0.to_le_bytes());
    }

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_set_selection_owner(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse SetSelectionOwner request: owner(4), selection(4), time(4)
    if data.len() < 12 {
        log::warn!("SetSelectionOwner request too short");
        return Ok(());
    }

    let owner = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let selection = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let time = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    log::debug!(
        "SetSelectionOwner: owner=0x{:x}, selection=0x{:x}, time={}",
        owner,
        selection,
        time
    );

    let mut server = server.lock().unwrap();
    server.set_selection_owner(
        crate::protocol::Atom::new(selection),
        crate::protocol::Window::new(owner),
        time,
    );

    // No reply for SetSelectionOwner
    Ok(())
}

fn handle_get_selection_owner(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse GetSelectionOwner request: selection(4)
    if data.len() < 4 {
        log::warn!("GetSelectionOwner request too short");
        return Ok(());
    }

    let selection = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("GetSelectionOwner: selection=0x{:x}", selection);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let owner = server.get_selection_owner(crate::protocol::Atom::new(selection));

    // Encode reply: reply(1), pad(1), sequence(2), length(4), owner(4), pad(20)
    let mut reply = vec![0u8; 32];
    reply[0] = 1; // Reply
    reply[2..4].copy_from_slice(&sequence.to_le_bytes());
    reply[4..8].copy_from_slice(&0u32.to_le_bytes()); // length = 0
    reply[8..12].copy_from_slice(&owner.id().get().to_le_bytes());

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_set_input_focus(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse SetInputFocus request: revert_to(1 in header), focus(4), time(4)
    if data.len() < 8 {
        log::warn!("SetInputFocus request too short");
        return Ok(());
    }

    let revert_to = header[1];
    let focus = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let time = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    log::debug!(
        "SetInputFocus: focus=0x{:x}, revert_to={}, time={}",
        focus,
        revert_to,
        time
    );

    // No reply for SetInputFocus
    // TODO: Actually set focus on backend
    Ok(())
}

fn handle_get_input_focus(
    stream: &mut TcpStream,
    header: &[u8],
    _data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("GetInputFocus");

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();

    // Return root window as focus for now
    let focus = server.root_window();
    let revert_to = 1u8; // RevertToPointerRoot

    // Encode and send reply
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_get_input_focus_reply(sequence, focus, revert_to);

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_create_pixmap(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse CreatePixmap request: depth(1 in header), pixmap(4), drawable(4), width(2), height(2)
    if data.len() < 12 {
        log::warn!("CreatePixmap request too short");
        return Ok(());
    }

    let depth = header[1];
    let pixmap = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let drawable = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let width = u16::from_le_bytes([data[8], data[9]]);
    let height = u16::from_le_bytes([data[10], data[11]]);

    log::debug!(
        "CreatePixmap: pixmap=0x{:x}, drawable=0x{:x}, {}x{}, depth={}",
        pixmap,
        drawable,
        width,
        height,
        depth
    );

    // TODO: Actually create pixmap in backend
    // No reply for CreatePixmap
    Ok(())
}

fn handle_free_pixmap(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse FreePixmap request: pixmap(4)
    if data.len() < 4 {
        log::warn!("FreePixmap request too short");
        return Ok(());
    }

    let pixmap = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("FreePixmap: pixmap=0x{:x}", pixmap);

    // TODO: Actually free pixmap in backend
    // No reply for FreePixmap
    Ok(())
}

fn handle_free_gc(
    _stream: &mut TcpStream,
    _header: &[u8],
    data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse FreeGC request: gc(4)
    if data.len() < 4 {
        log::warn!("FreeGC request too short");
        return Ok(());
    }

    let gc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("FreeGC: gc=0x{:x}", gc);

    // TODO: Actually free GC
    // No reply for FreeGC
    Ok(())
}

fn handle_clear_area(
    _stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse ClearArea request: exposures(1 in header), window(4), x(2), y(2), width(2), height(2)
    if data.len() < 12 {
        log::warn!("ClearArea request too short");
        return Ok(());
    }

    let exposures = header[1] != 0;
    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let x = i16::from_le_bytes([data[4], data[5]]);
    let y = i16::from_le_bytes([data[6], data[7]]);
    let width = u16::from_le_bytes([data[8], data[9]]);
    let height = u16::from_le_bytes([data[10], data[11]]);

    log::debug!(
        "ClearArea: window=0x{:x}, ({},{}) {}x{}, exposures={}",
        window,
        x,
        y,
        width,
        height,
        exposures
    );

    // TODO: Actually clear area using backend
    // No reply for ClearArea
    Ok(())
}

fn handle_grab_server(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("GrabServer (no-op)");
    // No reply for GrabServer
    // In a real X server this would prevent other clients from accessing the server
    // We just ignore it for now
    Ok(())
}

fn handle_ungrab_server(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("UngrabServer (no-op)");
    // No reply for UngrabServer
    Ok(())
}

fn handle_query_pointer(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse QueryPointer request: window(4)
    if data.len() < 4 {
        log::warn!("QueryPointer request too short");
        return Ok(());
    }

    let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    log::debug!("QueryPointer: window=0x{:x}", window);

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let server = server.lock().unwrap();
    let root = server.root_window();

    // Return the root window and (0,0) coordinates for now
    // TODO: Get actual pointer position from backend
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_query_pointer_reply(
        sequence,
        true,                            // same_screen
        root,                            // root
        crate::protocol::Window::new(0), // child (None)
        0,                               // root_x
        0,                               // root_y
        0,                               // win_x
        0,                               // win_y
        0,                               // mask (no buttons pressed)
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_translate_coordinates(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Parse TranslateCoordinates request: src_window(4), dst_window(4), src_x(2), src_y(2)
    if data.len() < 12 {
        log::warn!("TranslateCoordinates request too short");
        return Ok(());
    }

    let src_window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let dst_window = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let src_x = i16::from_le_bytes([data[8], data[9]]);
    let src_y = i16::from_le_bytes([data[10], data[11]]);

    log::debug!(
        "TranslateCoordinates: src=0x{:x}, dst=0x{:x}, ({},{})",
        src_window,
        dst_window,
        src_x,
        src_y
    );

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    let _server = server.lock().unwrap();

    // For now, just return the same coordinates (assume same coordinate space)
    // TODO: Actually translate coordinates based on window positions
    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_translate_coordinates_reply(
        sequence,
        true,                            // same_screen
        crate::protocol::Window::new(0), // child (None)
        src_x,                           // dst_x (same as src for now)
        src_y,                           // dst_y (same as src for now)
    );

    stream.write_all(&reply)?;

    Ok(())
}

fn handle_query_keymap(
    stream: &mut TcpStream,
    header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("QueryKeymap");

    // Get the sequence number from header
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    // Return empty keymap (no keys pressed)
    let keys = [0u8; 32];

    let encoder =
        crate::protocol::encoder::ProtocolEncoder::new(crate::protocol::ByteOrder::LSBFirst);
    let reply = encoder.encode_query_keymap_reply(sequence, &keys);

    stream.write_all(&reply)?;

    Ok(())
}
