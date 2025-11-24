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
            8 => handle_map_window(&mut stream, &header, &request_data)?,
            55 => handle_create_gc(&mut stream, &header, &request_data)?,
            56 => handle_change_gc(&mut stream, &header, &request_data)?,
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

// Minimal request handlers - just acknowledge requests
fn handle_create_window(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("CreateWindow request");
    Ok(())
}

fn handle_map_window(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("MapWindow request");
    Ok(())
}

fn handle_create_gc(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("CreateGC request");
    Ok(())
}

fn handle_change_gc(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("ChangeGC request");
    Ok(())
}

fn handle_poly_fill_rectangle(
    _stream: &mut TcpStream,
    _header: &[u8],
    _data: &[u8],
    _server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::debug!("PolyFillRectangle request");
    Ok(())
}
