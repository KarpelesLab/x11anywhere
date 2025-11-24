/// Test X11 server for protocol validation
///
/// This is a minimal X11 server that accepts connections and logs protocol messages.
/// Use it to test with real X11 clients like xcalc.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::fs;
use std::os::fd::{FromRawFd, AsRawFd};
use std::collections::HashMap;
use x11anywhere::protocol::*;
use x11anywhere::server::ExtensionInfo;

fn handle_client(mut stream: UnixStream, client_id: usize) {
    log::info!("[Client {}] Connected", client_id);

    // Read setup request
    let setup_request = match SetupRequest::parse(&mut stream) {
        Ok(req) => {
            log::info!("[Client {}] Setup request: {:?}", client_id, req);
            log::info!("[Client {}] Byte order: {:?}", client_id, req.byte_order);
            log::info!("[Client {}] Protocol: {}.{}",
                client_id, req.protocol_major_version, req.protocol_minor_version);
            req
        }
        Err(e) => {
            log::error!("[Client {}] Failed to parse setup request: {}", client_id, e);
            return;
        }
    };

    // Send setup reply
    let byte_order = setup_request.byte_order;

    // Create a minimal successful setup response
    let setup_response = SetupSuccess {
        protocol_major_version: 11,
        protocol_minor_version: 0,
        release_number: 0,
        resource_id_base: 0x00200000,  // Client ID base
        resource_id_mask: 0x001FFFFF,  // Client ID mask
        motion_buffer_size: 256,
        maximum_request_length: 65535,
        image_byte_order: ByteOrder::LSBFirst,
        bitmap_format_bit_order: ByteOrder::LSBFirst,
        bitmap_format_scanline_unit: 32,
        bitmap_format_scanline_pad: 32,
        min_keycode: 8,
        max_keycode: 255,
        vendor: "X11Anywhere Test Server".to_string(),
        pixmap_formats: vec![
            Format { depth: 1, bits_per_pixel: 1, scanline_pad: 32 },
            Format { depth: 24, bits_per_pixel: 32, scanline_pad: 32 },
        ],
        roots: vec![
            Screen {
                root: Window::new(1), // Root window ID
                default_colormap: Colormap::new(2),
                white_pixel: 0xFFFFFF,
                black_pixel: 0x000000,
                current_input_masks: 0,
                width_in_pixels: 1024,
                height_in_pixels: 768,
                width_in_millimeters: 270,
                height_in_millimeters: 203,
                min_installed_maps: 1,
                max_installed_maps: 1,
                root_visual: VisualID::new(0x20),
                backing_stores: BackingStore::NotUseful as u8,
                save_unders: false,
                root_depth: 24,
                allowed_depths: vec![
                    Depth {
                        depth: 24,
                        visuals: vec![
                            VisualType {
                                visual_id: VisualID::new(0x20),
                                class: 4,  // TrueColor
                                bits_per_rgb_value: 8,
                                colormap_entries: 256,
                                red_mask: 0xFF0000,
                                green_mask: 0x00FF00,
                                blue_mask: 0x0000FF,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    if let Err(e) = SetupResponse::Success(setup_response).encode(&mut stream, byte_order) {
        log::error!("[Client {}] Failed to send setup reply: {}", client_id, e);
        return;
    }

    log::info!("[Client {}] Sent setup reply successfully", client_id);

    // Create parser
    let parser = ProtocolParser::new(byte_order);
    let encoder = ProtocolEncoder::new(byte_order);

    // Register common extensions
    let mut extensions = HashMap::new();
    extensions.insert("BIG-REQUESTS".to_string(), ExtensionInfo {
        major_opcode: 133,
        first_event: 0,
        first_error: 0,
    });
    extensions.insert("XKEYBOARD".to_string(), ExtensionInfo {
        major_opcode: 135,
        first_event: 85,
        first_error: 0,
    });
    extensions.insert("RENDER".to_string(), ExtensionInfo {
        major_opcode: 138,
        first_event: 140,
        first_error: 0,
    });

    // Read and process requests
    let mut sequence: u16 = 0;
    let mut buffer = vec![0u8; 4096];

    loop {
        // Read request header first (4 bytes minimum)
        let bytes_read = match stream.read(&mut buffer[..4]) {
            Ok(0) => {
                log::info!("[Client {}] Connection closed", client_id);
                break;
            }
            Ok(n) if n < 4 => {
                log::warn!("[Client {}] Incomplete request header ({} bytes)", client_id, n);
                break;
            }
            Ok(_) => 4,
            Err(e) => {
                log::error!("[Client {}] Read error: {}", client_id, e);
                break;
            }
        };

        // Read the rest of the request based on length
        let length_field = match byte_order {
            ByteOrder::MSBFirst => u16::from_be_bytes([buffer[2], buffer[3]]),
            ByteOrder::LSBFirst => u16::from_le_bytes([buffer[2], buffer[3]]),
        };

        let request_size = (length_field as usize) * 4;
        if request_size < 4 || request_size > 4096 {
            log::error!("[Client {}] Invalid request size: {}", client_id, request_size);
            break;
        }

        // Read the rest of the request
        if request_size > 4 {
            match stream.read_exact(&mut buffer[4..request_size]) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("[Client {}] Failed to read request body: {}", client_id, e);
                    break;
                }
            }
        }

        sequence = sequence.wrapping_add(1);

        // Parse request
        match parser.parse_request(&buffer[..request_size]) {
            Ok((request, _)) => {
                log::info!("[Client {}] Request #{}: {:?}", client_id, sequence, request);

                // Handle some basic requests
                let reply = match request {
                    Request::GetWindowAttributes(req) => {
                        log::info!("[Client {}] GetWindowAttributes for window {}", client_id, req.window.id());
                        Some(encoder.encode_get_window_attributes_reply(
                            sequence,
                            VisualID::new(0x20),
                            WindowClass::InputOutput,
                            0, // bit_gravity
                            BackingStore::NotUseful,
                            0, // backing_planes
                            0, // backing_pixel
                            false, // save_under
                            true, // map_is_installed
                            MapState::Viewable,
                            false, // override_redirect
                            Colormap::new(2),
                            0xFFFFFF, // all_event_masks
                            0, // your_event_mask
                            0, // do_not_propagate_mask
                        ))
                    }
                    Request::GetGeometry(req) => {
                        log::info!("[Client {}] GetGeometry for drawable {}", client_id, req.drawable.id());
                        Some(encoder.encode_get_geometry_reply(
                            sequence,
                            24, // depth
                            Window::new(1), // root
                            0, 0, // x, y
                            1024, 768, // width, height
                            0, // border_width
                        ))
                    }
                    Request::QueryTree(req) => {
                        log::info!("[Client {}] QueryTree for window {}", client_id, req.window.id());
                        Some(encoder.encode_query_tree_reply(
                            sequence,
                            Window::new(1), // root
                            Window::new(1), // parent
                            &[], // no children
                        ))
                    }
                    Request::InternAtom(req) => {
                        log::info!("[Client {}] InternAtom: '{}' only_if_exists={}",
                            client_id, req.name, req.only_if_exists);
                        // Return a dummy atom ID
                        Some(encoder.encode_intern_atom_reply(sequence, Atom::new(100)))
                    }
                    Request::GetAtomName(req) => {
                        log::info!("[Client {}] GetAtomName: {}", client_id, req.atom.get());
                        Some(encoder.encode_get_atom_name_reply(sequence, "TEST_ATOM"))
                    }
                    Request::QueryExtension(req) => {
                        log::info!("[Client {}] QueryExtension: '{}'", client_id, req.name);
                        if let Some(ext) = extensions.get(&req.name) {
                            log::info!("[Client {}]   -> Present: opcode={}, first_event={}, first_error={}",
                                client_id, ext.major_opcode, ext.first_event, ext.first_error);
                            Some(encoder.encode_query_extension_reply(
                                sequence,
                                true,
                                ext.major_opcode,
                                ext.first_event,
                                ext.first_error,
                            ))
                        } else {
                            log::info!("[Client {}]   -> Not present", client_id);
                            Some(encoder.encode_query_extension_reply(sequence, false, 0, 0, 0))
                        }
                    }
                    Request::GetProperty(req) => {
                        log::info!("[Client {}] GetProperty: window={}, property={}",
                            client_id, req.window.id(), req.property.get());
                        // Return empty property
                        Some(encoder.encode_get_property_reply(
                            sequence,
                            0, // format (0 = no property)
                            Atom::NONE,
                            0, // bytes_after
                            &[],
                        ))
                    }
                    Request::CreateWindow(_) |
                    Request::MapWindow(_) |
                    Request::CreateGC(_) |
                    Request::CreatePixmap(_) |
                    Request::FreePixmap(_) |
                    Request::PutImage(_) |
                    Request::OpenFont(_) |
                    Request::CloseFont(_) |
                    Request::CreateGlyphCursor(_) |
                    Request::ChangeProperty(_) => {
                        // These don't return replies, just succeed silently
                        None
                    }
                    Request::GetInputFocus => {
                        log::info!("[Client {}] GetInputFocus", client_id);
                        // Return root window as focus, revert_to = PointerRoot (1)
                        Some(encoder.encode_get_input_focus_reply(
                            sequence,
                            Window::new(1), // Root window
                            1, // RevertTo::PointerRoot
                        ))
                    }
                    Request::AllocNamedColor(req) => {
                        log::info!("[Client {}] AllocNamedColor: colormap=0x{:08x}, name={}", client_id, req.colormap, req.name);
                        // Simple color allocation - return white for simplicity
                        let (r, g, b) = (0xFFFF, 0xFFFF, 0xFFFF);
                        Some(encoder.encode_alloc_named_color_reply(
                            sequence,
                            0xFFFFFF, // pixel value (white)
                            r, g, b,  // exact RGB
                            r, g, b,  // visual RGB (same as exact)
                        ))
                    }
                    Request::ExtensionRequest { opcode, data } => {
                        log::info!("[Client {}] Extension request: opcode={}", client_id, opcode);
                        // BIG-REQUESTS enable (opcode 133) returns maximum request length
                        if opcode == 133 {
                            let mut reply = vec![0u8; 32];
                            reply[0] = 1; // Reply type
                            // Write sequence number (little-endian for LSBFirst)
                            reply[2..4].copy_from_slice(&sequence.to_le_bytes());
                            // Write length (0 = 32 bytes total)
                            reply[4..8].copy_from_slice(&0u32.to_le_bytes());
                            // Write maximum request length
                            reply[8..12].copy_from_slice(&0x3FFFFFu32.to_le_bytes());
                            Some(reply)
                        } else if opcode == 135 && !data.is_empty() && data[0] == 1 {
                            // XKEYBOARD UseExtension (minor opcode 1)
                            log::info!("[Client {}]   -> XKEYBOARD UseExtension", client_id);
                            let mut reply = vec![0u8; 32];
                            reply[0] = 1; // Reply type
                            reply[1] = 1; // Supported = yes
                            reply[2..4].copy_from_slice(&sequence.to_le_bytes());
                            reply[4..8].copy_from_slice(&0u32.to_le_bytes()); // Length = 0
                            // Server version: 1.0
                            reply[8..10].copy_from_slice(&1u16.to_le_bytes()); // Major version
                            reply[10..12].copy_from_slice(&0u16.to_le_bytes()); // Minor version
                            Some(reply)
                        } else if opcode == 138 && data.len() >= 8 {
                            // RENDER QueryVersion (minor opcode 0)
                            log::info!("[Client {}]   -> RENDER QueryVersion", client_id);
                            let mut reply = vec![0u8; 32];
                            reply[0] = 1; // Reply type
                            reply[2..4].copy_from_slice(&sequence.to_le_bytes());
                            reply[4..8].copy_from_slice(&0u32.to_le_bytes()); // Length = 0
                            // Server version: 0.11 (same as client requested)
                            reply[8..12].copy_from_slice(&0u32.to_le_bytes()); // Major version
                            reply[12..16].copy_from_slice(&11u32.to_le_bytes()); // Minor version
                            Some(reply)
                        } else if opcode == 138 && data.is_empty() {
                            // RENDER QueryPictFormats (minor opcode 1)
                            log::info!("[Client {}]   -> RENDER QueryPictFormats", client_id);
                            let mut reply = vec![0u8; 32];
                            reply[0] = 1; // Reply type
                            reply[2..4].copy_from_slice(&sequence.to_le_bytes());
                            reply[4..8].copy_from_slice(&0u32.to_le_bytes()); // Length = 0
                            // Return empty format list (no formats supported)
                            reply[8..12].copy_from_slice(&0u32.to_le_bytes()); // num_formats = 0
                            reply[12..16].copy_from_slice(&0u32.to_le_bytes()); // num_screens = 0
                            reply[16..20].copy_from_slice(&0u32.to_le_bytes()); // num_depths = 0
                            reply[20..24].copy_from_slice(&0u32.to_le_bytes()); // num_visuals = 0
                            reply[24..28].copy_from_slice(&0u32.to_le_bytes()); // num_subpixel = 0
                            Some(reply)
                        } else {
                            // Other extension requests don't reply
                            None
                        }
                    }
                    Request::NoOperation => {
                        log::info!("[Client {}] NoOperation", client_id);
                        None
                    }
                    _ => {
                        log::warn!("[Client {}] Unhandled request type", client_id);
                        None
                    }
                };

                // Send reply if there is one
                if let Some(reply_data) = reply {
                    if let Err(e) = stream.write_all(&reply_data) {
                        log::error!("[Client {}] Failed to send reply: {}", client_id, e);
                        break;
                    }
                }
            }
            Err(e) => {
                log::error!("[Client {}] Failed to parse request: {}", client_id, e);
                // Send error response
                let mut error_buffer = [0u8; 32];
                e.encode(&mut error_buffer);
                if let Err(e) = stream.write_all(&error_buffer) {
                    log::error!("[Client {}] Failed to send error: {}", client_id, e);
                }
                break;
            }
        }
    }

    log::info!("[Client {}] Disconnected", client_id);
}

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let display = 99;
    let socket_dir = "/tmp/.X11-unix";

    log::info!("Starting X11 test server on display :{}", display);
    log::info!("Clients can connect with: DISPLAY=:{} <app>", display);

    // Create socket directory if it doesn't exist
    fs::create_dir_all(socket_dir).expect("Failed to create socket directory");

    // On Linux, X11 uses abstract sockets (prefixed with \0)
    // Use nix crate to create abstract socket
    #[cfg(target_os = "linux")]
    {
        use nix::sys::socket::*;

        // Create socket
        let socket_fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        ).expect("Failed to create socket");

        // Create abstract socket address
        let abstract_name = format!("/tmp/.X11-unix/X{}", display);
        let addr = UnixAddr::new_abstract(abstract_name.as_bytes())
            .expect("Failed to create abstract address");

        // Bind socket
        bind(socket_fd.as_raw_fd(), &addr)
            .expect("Failed to bind abstract socket");

        // Listen
        listen(&socket_fd, Backlog::new(128).unwrap())
            .expect("Failed to listen on socket");

        // Convert to UnixListener
        let listener = unsafe { UnixListener::from_raw_fd(socket_fd.as_raw_fd()) };
        std::mem::forget(socket_fd); // Don't close the fd

        log::info!("Listening on abstract Unix socket: @/tmp/.X11-unix/X{}", display);

        let mut client_counter = 0;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    client_counter += 1;
                    let client_id = client_counter;

                    thread::spawn(move || {
                        handle_client(stream, client_id);
                    });
                }
                Err(e) => {
                    log::error!("Connection error: {}", e);
                }
            }
        }
        return;
    }

    // Fallback for non-Linux: use filesystem socket
    #[cfg(not(target_os = "linux"))]
    {
        let socket_path = format!("{}/X{}", socket_dir, display);
        let _ = fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path)
            .expect("Failed to bind to Unix socket");

        log::info!("Listening on filesystem Unix socket: {}", socket_path);

        let mut client_counter = 0;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    client_counter += 1;
                    let client_id = client_counter;

                    thread::spawn(move || {
                        handle_client(stream, client_id);
                    });
                }
                Err(e) => {
                    log::error!("Connection error: {}", e);
                }
            }
        }
    }
}
