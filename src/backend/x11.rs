//! X11 Backend - Passthrough to a real X11 server
//!
//! This backend connects to a real X11 server and forwards all protocol messages.
//! It can be used for:
//! - Nested X11 server (like Xephyr)
//! - Protocol debugging and logging
//! - Testing the protocol implementation

use super::*;
use crate::protocol::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct X11Backend {
    display: String,
    connection: Option<UnixStream>,
    setup_info: Option<SetupSuccess>,
    byte_order: ByteOrder,

    // Resource ID mapping (our IDs -> real server IDs)
    window_map: Arc<Mutex<HashMap<usize, u32>>>,
    pixmap_map: Arc<Mutex<HashMap<usize, u32>>>,
    gc_map: Arc<Mutex<HashMap<usize, u32>>>,

    // For generating our own resource IDs
    next_resource_id: usize,
    next_gc_id: usize,
    resource_id_base: u32,
    resource_id_mask: u32,

    // Default font for text rendering
    default_font_id: Option<u32>,

    debug: bool,
}

impl X11Backend {
    pub fn new(target_display: &str) -> Self {
        Self {
            display: target_display.to_string(),
            connection: None,
            setup_info: None,
            byte_order: ByteOrder::LSBFirst,
            window_map: Arc::new(Mutex::new(HashMap::new())),
            pixmap_map: Arc::new(Mutex::new(HashMap::new())),
            gc_map: Arc::new(Mutex::new(HashMap::new())),
            next_resource_id: 1,
            next_gc_id: 1,
            resource_id_base: 0,
            resource_id_mask: 0,
            default_font_id: None,
            debug: true,
        }
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Allocate a resource ID on the connected X server
    fn allocate_server_resource_id(&mut self) -> u32 {
        let id = self.resource_id_base | (self.next_resource_id as u32 & self.resource_id_mask);
        self.next_resource_id += 1;
        id
    }

    /// Send a request to the X server
    fn send_request(&mut self, data: &[u8]) -> BackendResult<()> {
        if let Some(ref mut conn) = self.connection {
            conn.write_all(data)
                .map_err(|e| format!("Failed to send X11 request: {}", e))?;
            if self.debug {
                log::debug!("Sent {} bytes to X server", data.len());
            }
            Ok(())
        } else {
            Err("Not connected to X server".into())
        }
    }

    /// Send a request and read the reply from the X server
    fn send_request_with_reply(&mut self, data: &[u8]) -> BackendResult<Vec<u8>> {
        // Send the request
        self.send_request(data)?;
        self.flush()?;

        // Read the reply header (32 bytes)
        let conn = self
            .connection
            .as_mut()
            .ok_or("Not connected to X server")?;
        let mut header = [0u8; 32];
        conn.read_exact(&mut header)
            .map_err(|e| format!("Failed to read reply header: {}", e))?;

        let reply_type = header[0];
        if reply_type == 0 {
            // Error response
            let error_code = header[1];
            return Err(format!("X11 error: code={}", error_code).into());
        }
        if reply_type != 1 {
            return Err(format!("Unexpected reply type: {}", reply_type).into());
        }

        // Get the additional data length (in 4-byte units)
        let additional_length =
            u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let additional_bytes = additional_length * 4;

        // Read additional data if any
        let mut reply_data = header.to_vec();
        if additional_bytes > 0 {
            let mut additional = vec![0u8; additional_bytes];
            conn.read_exact(&mut additional)
                .map_err(|e| format!("Failed to read reply data: {}", e))?;
            reply_data.extend_from_slice(&additional);
        }

        if self.debug {
            log::debug!("Received {} byte reply from X server", reply_data.len());
        }

        Ok(reply_data)
    }

    /// Create a GC on the server
    fn create_server_gc(&mut self, drawable: u32, gc_id: u32, gc: &BackendGC) -> BackendResult<()> {
        // Build CreateGC request (opcode 55)
        let mut req = Vec::new();
        req.push(55); // Opcode: CreateGC
        req.push(0); // Padding
        req.extend_from_slice(&4u16.to_le_bytes()); // Length placeholder
        req.extend_from_slice(&gc_id.to_le_bytes()); // cid
        req.extend_from_slice(&drawable.to_le_bytes()); // drawable

        // Value mask and value list
        let mut value_mask = 0u32;
        let mut value_list = Vec::new();

        // Foreground (bit 2)
        value_mask |= 0x00000004;
        value_list.extend_from_slice(&gc.foreground.to_le_bytes());

        // Background (bit 3)
        value_mask |= 0x00000008;
        value_list.extend_from_slice(&gc.background.to_le_bytes());

        req.extend_from_slice(&value_mask.to_le_bytes());
        req.extend_from_slice(&value_list);

        // Update length
        let len_words = req.len().div_ceil(4);
        req[2..4].copy_from_slice(&(len_words as u16).to_le_bytes());

        // Pad to 4-byte boundary
        while req.len() % 4 != 0 {
            req.push(0);
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Created GC: id=0x{:x}, fg=0x{:x}, bg=0x{:x}",
                gc_id,
                gc.foreground,
                gc.background
            );
        }

        Ok(())
    }

    /// Change a GC on the server
    fn change_server_gc(&mut self, gc_id: u32, gc: &BackendGC) -> BackendResult<()> {
        // Build ChangeGC request (opcode 56)
        let mut req = Vec::new();
        req.push(56); // Opcode: ChangeGC
        req.push(0); // Padding
        req.extend_from_slice(&3u16.to_le_bytes()); // Length placeholder
        req.extend_from_slice(&gc_id.to_le_bytes()); // gc

        // Value mask and value list
        let mut value_mask = 0u32;
        let mut value_list = Vec::new();

        // Foreground (bit 2)
        value_mask |= 0x00000004;
        value_list.extend_from_slice(&gc.foreground.to_le_bytes());

        // Background (bit 3)
        value_mask |= 0x00000008;
        value_list.extend_from_slice(&gc.background.to_le_bytes());

        req.extend_from_slice(&value_mask.to_le_bytes());
        req.extend_from_slice(&value_list);

        // Update length
        let len_words = req.len().div_ceil(4);
        req[2..4].copy_from_slice(&(len_words as u16).to_le_bytes());

        // Pad to 4-byte boundary
        while req.len() % 4 != 0 {
            req.push(0);
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Changed GC: id=0x{:x}, fg=0x{:x}, bg=0x{:x}",
                gc_id,
                gc.foreground,
                gc.background
            );
        }

        Ok(())
    }

    /// Open a default font for text rendering
    fn open_default_font(&mut self) -> BackendResult<()> {
        let font_id = self.allocate_server_resource_id();

        // Try to open "fixed" font - a common fallback font on X11 systems
        let font_name = b"fixed";
        let name_len = font_name.len();
        let name_pad = (4 - (name_len % 4)) % 4;

        // Build OpenFont request (opcode 45)
        // Format: opcode(1), pad(1), length(2), fid(4), name_length(2), pad(2), name
        let mut req = Vec::new();
        req.push(45); // Opcode: OpenFont
        req.push(0); // Padding
        let length = 3 + (name_len + name_pad) / 4;
        req.extend_from_slice(&(length as u16).to_le_bytes());
        req.extend_from_slice(&font_id.to_le_bytes());
        req.extend_from_slice(&(name_len as u16).to_le_bytes());
        req.extend_from_slice(&[0, 0]); // padding
        req.extend_from_slice(font_name);
        req.extend(std::iter::repeat_n(0u8, name_pad));

        self.send_request(&req)?;
        self.default_font_id = Some(font_id);

        if self.debug {
            log::debug!("Opened default font 'fixed' with id 0x{:x}", font_id);
        }

        Ok(())
    }

    /// Create a GC on the server with optional font
    fn create_server_gc_with_font(
        &mut self,
        drawable: u32,
        gc_id: u32,
        gc: &BackendGC,
        font_id: Option<u32>,
    ) -> BackendResult<()> {
        // Build CreateGC request (opcode 55)
        let mut req = Vec::new();
        req.push(55); // Opcode: CreateGC
        req.push(0); // Padding
        req.extend_from_slice(&4u16.to_le_bytes()); // Length placeholder
        req.extend_from_slice(&gc_id.to_le_bytes()); // cid
        req.extend_from_slice(&drawable.to_le_bytes()); // drawable

        // Value mask and value list
        let mut value_mask = 0u32;
        let mut value_list = Vec::new();

        // Foreground (bit 2)
        value_mask |= 0x00000004;
        value_list.extend_from_slice(&gc.foreground.to_le_bytes());

        // Background (bit 3)
        value_mask |= 0x00000008;
        value_list.extend_from_slice(&gc.background.to_le_bytes());

        // Font (bit 14)
        if let Some(fid) = font_id {
            value_mask |= 0x00004000;
            value_list.extend_from_slice(&fid.to_le_bytes());
        }

        req.extend_from_slice(&value_mask.to_le_bytes());
        req.extend_from_slice(&value_list);

        // Update length
        let len_words = req.len().div_ceil(4);
        req[2..4].copy_from_slice(&(len_words as u16).to_le_bytes());

        // Pad to 4-byte boundary
        while req.len() % 4 != 0 {
            req.push(0);
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Created GC with font: id=0x{:x}, fg=0x{:x}, font={:?}",
                gc_id,
                gc.foreground,
                font_id
            );
        }

        Ok(())
    }

    fn connect_to_display(&mut self) -> BackendResult<()> {
        // Parse display number (format: :display.screen or :display)
        let display_str = self.display.trim_start_matches(':');
        // Split off the screen number if present (e.g., "0.0" -> "0")
        let display_num_str = display_str.split('.').next().unwrap_or(display_str);
        let display_num: usize = display_num_str
            .parse()
            .map_err(|_| format!("Invalid display number: {}", self.display))?;

        // Connect via abstract socket on Linux
        #[cfg(target_os = "linux")]
        let stream = {
            use nix::sys::socket::*;
            use std::os::fd::{AsRawFd, FromRawFd};

            // Create socket
            let socket_fd = socket(
                AddressFamily::Unix,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )
            .map_err(|e| format!("Failed to create socket: {}", e))?;

            // Create abstract socket address
            let abstract_name = format!("/tmp/.X11-unix/X{}", display_num);
            let addr = UnixAddr::new_abstract(abstract_name.as_bytes())
                .map_err(|e| format!("Failed to create abstract address: {}", e))?;

            // Connect
            if let Err(e) = connect(socket_fd.as_raw_fd(), &addr) {
                let _ = unsafe { UnixStream::from_raw_fd(socket_fd.as_raw_fd()) }; // Clean up
                return Err(format!("Failed to connect to display {}: {}", self.display, e).into());
            }

            // Convert to UnixStream
            let stream = unsafe { UnixStream::from_raw_fd(socket_fd.as_raw_fd()) };
            std::mem::forget(socket_fd); // Don't close the fd
            stream
        };

        #[cfg(not(target_os = "linux"))]
        let stream = {
            let path = format!("/tmp/.X11-unix/X{}", display_num);
            UnixStream::connect(&path)
                .map_err(|e| format!("Failed to connect to display {}: {}", self.display, e))?
        };

        self.connection = Some(stream);

        if self.debug {
            log::debug!("X11 backend connected to display {}", self.display);
        }

        Ok(())
    }

    fn read_xauthority(&self, display_num: usize) -> BackendResult<(String, Vec<u8>)> {
        // Get .Xauthority path from env or use default
        let xauth_path = std::env::var("XAUTHORITY")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".Xauthority"))
            })
            .ok_or("Could not find .Xauthority file")?;

        if self.debug {
            log::debug!("Reading auth from: {:?}", xauth_path);
        }

        let mut file = std::fs::File::open(&xauth_path)
            .map_err(|e| format!("Failed to open .Xauthority: {}", e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read .Xauthority: {}", e))?;

        let mut offset = 0;
        let display_str = format!("{}", display_num);

        // Parse .Xauthority entries
        while offset < data.len() {
            if offset + 2 > data.len() {
                break;
            }
            let family = u16::from_be_bytes([data[offset], data[offset + 1]]);
            offset += 2;

            // Read address
            if offset + 2 > data.len() {
                break;
            }
            let addr_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            if offset + addr_len > data.len() {
                break;
            }
            let _address = &data[offset..offset + addr_len];
            offset += addr_len;

            // Read display number
            if offset + 2 > data.len() {
                break;
            }
            let num_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            if offset + num_len > data.len() {
                break;
            }
            let number = String::from_utf8_lossy(&data[offset..offset + num_len]).to_string();
            offset += num_len;

            // Read auth name
            if offset + 2 > data.len() {
                break;
            }
            let name_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            if offset + name_len > data.len() {
                break;
            }
            let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
            offset += name_len;

            // Read auth data
            if offset + 2 > data.len() {
                break;
            }
            let data_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            if offset + data_len > data.len() {
                break;
            }
            let auth_data = data[offset..offset + data_len].to_vec();
            offset += data_len;

            // Check if this entry matches our display
            // Family 0x0100 = FamilyLocal, 0x0000 = FamilyInternet
            if (family == 0x0100 || family == 0x0000) && number == display_str {
                if self.debug {
                    log::debug!("Found auth: {} ({} bytes)", name, auth_data.len());
                }
                return Ok((name, auth_data));
            }
        }

        // No auth found - return empty
        if self.debug {
            log::debug!("No auth entry found for display {}", display_num);
        }
        Ok((String::new(), Vec::new()))
    }

    fn perform_handshake(&mut self) -> BackendResult<()> {
        // Get display number for auth lookup
        let display_num: usize = self.display.trim_start_matches(':').parse().unwrap_or(0);

        // Try to get auth from .Xauthority (before borrowing connection)
        let (auth_name, auth_data) = self.read_xauthority(display_num).unwrap_or_else(|e| {
            if self.debug {
                log::debug!("Could not read auth: {}", e);
            }
            (String::new(), Vec::new())
        });

        let stream = self.connection.as_mut().ok_or("Not connected")?;

        // Send setup request
        let mut setup_bytes = Vec::new();
        setup_bytes.push(b'l'); // LSB first
        setup_bytes.push(0); // Padding
        setup_bytes.extend_from_slice(&11u16.to_le_bytes()); // Protocol major
        setup_bytes.extend_from_slice(&0u16.to_le_bytes()); // Protocol minor

        // Auth protocol name
        let auth_name_len = auth_name.len() as u16;
        setup_bytes.extend_from_slice(&auth_name_len.to_le_bytes());

        // Auth protocol data
        let auth_data_len = auth_data.len() as u16;
        setup_bytes.extend_from_slice(&auth_data_len.to_le_bytes());

        setup_bytes.extend_from_slice(&[0u8; 2]); // Padding

        // Add auth name (padded to 4 bytes)
        setup_bytes.extend_from_slice(auth_name.as_bytes());
        let auth_name_pad = (4 - (auth_name.len() % 4)) % 4;
        setup_bytes.extend_from_slice(&vec![0u8; auth_name_pad]);

        // Add auth data (padded to 4 bytes)
        setup_bytes.extend_from_slice(&auth_data);
        let auth_data_pad = (4 - (auth_data.len() % 4)) % 4;
        setup_bytes.extend_from_slice(&vec![0u8; auth_data_pad]);

        stream
            .write_all(&setup_bytes)
            .map_err(|e| format!("Failed to send setup: {}", e))?;

        if self.debug {
            log::debug!("Sent setup request to real X server");
        }

        // Read setup reply
        let mut header = [0u8; 8];
        stream
            .read_exact(&mut header)
            .map_err(|e| format!("Failed to read setup reply: {}", e))?;

        let status = header[0];
        let additional_length = u16::from_le_bytes([header[6], header[7]]);

        if status != 1 {
            // Read failure reason
            let data_len = (additional_length as usize) * 4;
            let mut data = vec![0u8; data_len];
            stream.read_exact(&mut data).ok();
            let reason = String::from_utf8_lossy(&data[..header[1].min(data.len() as u8) as usize]);
            return Err(format!("X server rejected connection: {}", reason).into());
        }

        // Read success data
        let data_len = (additional_length as usize) * 4;
        let mut data = vec![0u8; data_len];
        stream
            .read_exact(&mut data)
            .map_err(|e| format!("Failed to read setup data: {}", e))?;

        if self.debug {
            log::debug!(
                "Real X server sent {} byte header + {} bytes data:",
                header.len(),
                data.len()
            );
            log::debug!("Header: {:02x?}", header);
            // Dump first 64 bytes of data
            for (i, chunk) in data.chunks(16).take(4).enumerate() {
                let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
                log::debug!("  {:04x}: {}", i * 16, hex);
            }
        }

        // Parse the setup success
        let setup = self.parse_setup_success(&header, &data)?;

        if self.debug {
            log::debug!("Setup successful:");
            log::debug!("  Resource ID base: 0x{:08x}", setup.resource_id_base);
            log::debug!("  Resource ID mask: 0x{:08x}", setup.resource_id_mask);
            log::debug!("  Vendor: {}", setup.vendor);
            log::debug!("  Screens: {}", setup.roots.len());
        }

        self.resource_id_base = setup.resource_id_base;
        self.resource_id_mask = setup.resource_id_mask;
        self.setup_info = Some(setup);

        Ok(())
    }

    fn parse_setup_success(&self, header: &[u8], data: &[u8]) -> BackendResult<SetupSuccess> {
        let protocol_major = u16::from_le_bytes([header[2], header[3]]);
        let protocol_minor = u16::from_le_bytes([header[4], header[5]]);

        let release_number = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let resource_id_base = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let resource_id_mask = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let motion_buffer_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let vendor_len = u16::from_le_bytes([data[16], data[17]]) as usize;
        let max_request_length = u16::from_le_bytes([data[18], data[19]]);
        let num_screens = data[20];
        let num_formats = data[21];
        let image_byte_order = if data[22] == 0 {
            ByteOrder::LSBFirst
        } else {
            ByteOrder::MSBFirst
        };
        let bitmap_bit_order = if data[23] == 0 {
            ByteOrder::LSBFirst
        } else {
            ByteOrder::MSBFirst
        };
        let bitmap_scanline_unit = data[24];
        let bitmap_scanline_pad = data[25];
        let min_keycode = data[26];
        let max_keycode = data[27];

        // Vendor string
        let vendor = if data.len() > 32 + vendor_len {
            String::from_utf8_lossy(&data[32..32 + vendor_len]).to_string()
        } else {
            String::new()
        };

        // Parse formats
        let vendor_padded = padded_len(vendor_len);
        let mut offset = 32 + vendor_padded;
        let mut formats = Vec::new();

        for _ in 0..num_formats {
            if offset + 8 <= data.len() {
                formats.push(Format {
                    depth: data[offset],
                    bits_per_pixel: data[offset + 1],
                    scanline_pad: data[offset + 2],
                });
                offset += 8;
            }
        }

        // Parse screens
        let mut roots = Vec::new();
        for _ in 0..num_screens {
            if offset + 40 <= data.len() {
                let root = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                let default_colormap = u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                let white_pixel = u32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]);
                let black_pixel = u32::from_le_bytes([
                    data[offset + 12],
                    data[offset + 13],
                    data[offset + 14],
                    data[offset + 15],
                ]);
                let current_input_masks = u32::from_le_bytes([
                    data[offset + 16],
                    data[offset + 17],
                    data[offset + 18],
                    data[offset + 19],
                ]);
                let width = u16::from_le_bytes([data[offset + 20], data[offset + 21]]);
                let height = u16::from_le_bytes([data[offset + 22], data[offset + 23]]);
                let width_mm = u16::from_le_bytes([data[offset + 24], data[offset + 25]]);
                let height_mm = u16::from_le_bytes([data[offset + 26], data[offset + 27]]);
                let min_maps = u16::from_le_bytes([data[offset + 28], data[offset + 29]]);
                let max_maps = u16::from_le_bytes([data[offset + 30], data[offset + 31]]);
                let root_visual = u32::from_le_bytes([
                    data[offset + 32],
                    data[offset + 33],
                    data[offset + 34],
                    data[offset + 35],
                ]);
                let backing_stores = data[offset + 36];
                let save_unders = data[offset + 37] != 0;
                let root_depth = data[offset + 38];
                let num_depths = data[offset + 39];

                offset += 40;

                // Parse depths and visuals
                let mut depths = Vec::new();
                for _ in 0..num_depths {
                    if offset + 8 <= data.len() {
                        let depth = data[offset];
                        let num_visuals = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
                        offset += 8;

                        let mut visuals = Vec::new();
                        for _ in 0..num_visuals {
                            if offset + 24 <= data.len() {
                                visuals.push(VisualType {
                                    visual_id: VisualID::new(u32::from_le_bytes([
                                        data[offset],
                                        data[offset + 1],
                                        data[offset + 2],
                                        data[offset + 3],
                                    ])),
                                    class: data[offset + 4],
                                    bits_per_rgb_value: data[offset + 5],
                                    colormap_entries: u16::from_le_bytes([
                                        data[offset + 6],
                                        data[offset + 7],
                                    ]),
                                    red_mask: u32::from_le_bytes([
                                        data[offset + 8],
                                        data[offset + 9],
                                        data[offset + 10],
                                        data[offset + 11],
                                    ]),
                                    green_mask: u32::from_le_bytes([
                                        data[offset + 12],
                                        data[offset + 13],
                                        data[offset + 14],
                                        data[offset + 15],
                                    ]),
                                    blue_mask: u32::from_le_bytes([
                                        data[offset + 16],
                                        data[offset + 17],
                                        data[offset + 18],
                                        data[offset + 19],
                                    ]),
                                });
                                offset += 24;
                            }
                        }

                        depths.push(Depth { depth, visuals });
                    }
                }

                roots.push(Screen {
                    root: Window::new(root),
                    default_colormap: Colormap::new(default_colormap),
                    white_pixel,
                    black_pixel,
                    current_input_masks,
                    width_in_pixels: width,
                    height_in_pixels: height,
                    width_in_millimeters: width_mm,
                    height_in_millimeters: height_mm,
                    min_installed_maps: min_maps,
                    max_installed_maps: max_maps,
                    root_visual: VisualID::new(root_visual),
                    backing_stores,
                    save_unders,
                    root_depth,
                    allowed_depths: depths,
                });
            }
        }

        Ok(SetupSuccess {
            protocol_major_version: protocol_major,
            protocol_minor_version: protocol_minor,
            release_number,
            resource_id_base,
            resource_id_mask,
            motion_buffer_size,
            maximum_request_length: max_request_length,
            image_byte_order,
            bitmap_format_bit_order: bitmap_bit_order,
            bitmap_format_scanline_unit: bitmap_scanline_unit,
            bitmap_format_scanline_pad: bitmap_scanline_pad,
            min_keycode,
            max_keycode,
            vendor,
            pixmap_formats: formats,
            roots,
        })
    }
}

impl Backend for X11Backend {
    fn init(&mut self) -> BackendResult<()> {
        self.connect_to_display()?;
        self.perform_handshake()?;
        // Open a default font for text rendering
        // This may fail on systems without the "fixed" font, but we continue anyway
        if let Err(e) = self.open_default_font() {
            if self.debug {
                log::debug!("Could not open default font: {}", e);
            }
        }
        Ok(())
    }

    fn get_screen_info(&self) -> BackendResult<ScreenInfo> {
        let setup = self.setup_info.as_ref().ok_or("Not initialized")?;

        let screen = &setup.roots[0];

        Ok(ScreenInfo {
            width: screen.width_in_pixels,
            height: screen.height_in_pixels,
            width_mm: screen.width_in_millimeters,
            height_mm: screen.height_in_millimeters,
            root_visual: screen.root_visual,
            root_depth: screen.root_depth,
            white_pixel: screen.white_pixel,
            black_pixel: screen.black_pixel,
        })
    }

    fn get_visuals(&self) -> BackendResult<Vec<VisualInfo>> {
        let setup = self.setup_info.as_ref().ok_or("Not initialized")?;

        let screen = &setup.roots[0];
        let mut visuals = Vec::new();

        for depth in &screen.allowed_depths {
            for visual in &depth.visuals {
                visuals.push(VisualInfo {
                    visual_id: visual.visual_id,
                    class: visual.class,
                    bits_per_rgb: visual.bits_per_rgb_value,
                    colormap_entries: visual.colormap_entries,
                    red_mask: visual.red_mask,
                    green_mask: visual.green_mask,
                    blue_mask: visual.blue_mask,
                });
            }
        }

        Ok(visuals)
    }

    fn create_window(&mut self, params: WindowParams) -> BackendResult<BackendWindow> {
        // Allocate IDs
        let our_id = self.next_resource_id;
        self.next_resource_id += 1;
        let server_wid = self.allocate_server_resource_id();

        // Get parent window ID (root is from setup)
        let parent_id = if let Some(parent) = params.parent {
            *self
                .window_map
                .lock()
                .unwrap()
                .get(&parent.0)
                .unwrap_or(&server_wid)
        } else {
            // Use root window from setup
            if let Some(ref setup) = self.setup_info {
                setup.roots[0].root.id().get()
            } else {
                return Err("Not initialized".into());
            }
        };

        // Get visual ID
        let visual_id = if let Some(ref setup) = self.setup_info {
            setup.roots[0].root_visual.get()
        } else {
            return Err("Not initialized".into());
        };

        // Build CreateWindow request (opcode 1)
        let mut req = Vec::new();
        req.push(1); // Opcode: CreateWindow
        req.push(24); // Depth
        req.extend_from_slice(&(8u16.to_le_bytes())); // Length in 4-byte units (8 = 32 bytes)
        req.extend_from_slice(&server_wid.to_le_bytes()); // wid
        req.extend_from_slice(&parent_id.to_le_bytes()); // parent
        req.extend_from_slice(&params.x.to_le_bytes()); // x
        req.extend_from_slice(&params.y.to_le_bytes()); // y
        req.extend_from_slice(&params.width.to_le_bytes()); // width
        req.extend_from_slice(&params.height.to_le_bytes()); // height
        req.extend_from_slice(&params.border_width.to_le_bytes()); // border_width
        req.extend_from_slice(&1u16.to_le_bytes()); // class: InputOutput
        req.extend_from_slice(&visual_id.to_le_bytes()); // visual

        // Value mask and value list
        let mut value_mask = 0u32;
        let mut value_list = Vec::new();

        if let Some(bg) = params.background_pixel {
            value_mask |= 0x00000002; // CWBackPixel
            value_list.extend_from_slice(&bg.to_le_bytes());
        }

        if params.event_mask != 0 {
            value_mask |= 0x00000800; // CWEventMask
            value_list.extend_from_slice(&params.event_mask.to_le_bytes());
        }

        req.extend_from_slice(&value_mask.to_le_bytes());
        req.extend_from_slice(&value_list);

        // Update length field
        let total_len = req.len();
        let len_words = total_len.div_ceil(4);
        req[2..4].copy_from_slice(&(len_words as u16).to_le_bytes());

        // Pad to 4-byte boundary
        while req.len() % 4 != 0 {
            req.push(0);
        }

        self.send_request(&req)?;

        // Store mapping
        self.window_map.lock().unwrap().insert(our_id, server_wid);

        if self.debug {
            log::debug!(
                "Created window: our_id={}, server_id=0x{:x}",
                our_id,
                server_wid
            );
        }

        Ok(BackendWindow(our_id))
    }

    fn destroy_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn map_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        // Get server window ID
        let server_wid = *self
            .window_map
            .lock()
            .unwrap()
            .get(&window.0)
            .ok_or("Window not found")?;

        // Build MapWindow request (opcode 8)
        let mut req = Vec::new();
        req.push(8); // Opcode: MapWindow
        req.push(0); // Padding
        req.extend_from_slice(&2u16.to_le_bytes()); // Length: 2 words = 8 bytes
        req.extend_from_slice(&server_wid.to_le_bytes()); // window

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Mapped window: our_id={}, server_id=0x{:x}",
                window.0,
                server_wid
            );
        }

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

    fn raise_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        // Get X server window ID
        let server_wid = *self
            .window_map
            .lock()
            .unwrap()
            .get(&window.0)
            .ok_or("Window not found")?;

        // ConfigureWindow (opcode 12) with stack-mode = Above (0)
        // Value mask bit 6 (0x40) is stack-mode
        let mut req = Vec::new();
        req.push(12); // Opcode: ConfigureWindow
        req.push(0); // Unused
        req.extend_from_slice(&4u16.to_le_bytes()); // Length: 4 words = 16 bytes
        req.extend_from_slice(&server_wid.to_le_bytes()); // Window
        req.extend_from_slice(&0x0040u16.to_le_bytes()); // Value mask: stack-mode
        req.extend_from_slice(&[0u8; 2]); // Padding
        req.extend_from_slice(&0u32.to_le_bytes()); // Stack mode: Above = 0

        self.send_request(&req)?;
        self.flush()?;

        if self.debug {
            log::debug!("Raised window 0x{:x}", server_wid);
        }
        Ok(())
    }

    fn lower_window(&mut self, window: BackendWindow) -> BackendResult<()> {
        // Get X server window ID
        let server_wid = *self
            .window_map
            .lock()
            .unwrap()
            .get(&window.0)
            .ok_or("Window not found")?;

        // ConfigureWindow (opcode 12) with stack-mode = Below (1)
        // Value mask bit 6 (0x40) is stack-mode
        let mut req = Vec::new();
        req.push(12); // Opcode: ConfigureWindow
        req.push(0); // Unused
        req.extend_from_slice(&4u16.to_le_bytes()); // Length: 4 words = 16 bytes
        req.extend_from_slice(&server_wid.to_le_bytes()); // Window
        req.extend_from_slice(&0x0040u16.to_le_bytes()); // Value mask: stack-mode
        req.extend_from_slice(&[0u8; 2]); // Padding
        req.extend_from_slice(&1u32.to_le_bytes()); // Stack mode: Below = 1

        self.send_request(&req)?;
        self.flush()?;

        if self.debug {
            log::debug!("Lowered window 0x{:x}", server_wid);
        }
        Ok(())
    }

    fn set_window_title(&mut self, window: BackendWindow, title: &str) -> BackendResult<()> {
        // Get X server window ID
        let server_wid = *self
            .window_map
            .lock()
            .unwrap()
            .get(&window.0)
            .ok_or("Window not found")?;

        // ChangeProperty (opcode 18) to set WM_NAME (atom 39)
        // Format: opcode(1), mode(1), length(2), window(4), property(4), type(4),
        //         format(1), pad(3), data_length(4), data(...)
        let title_bytes = title.as_bytes();
        let padded_len = (title_bytes.len() + 3) & !3; // Pad to 4-byte boundary
        let total_len = (24 + padded_len) / 4;

        let mut req = Vec::new();
        req.push(18); // Opcode: ChangeProperty
        req.push(0); // Mode: Replace = 0
        req.extend_from_slice(&(total_len as u16).to_le_bytes()); // Length
        req.extend_from_slice(&server_wid.to_le_bytes()); // Window
        req.extend_from_slice(&39u32.to_le_bytes()); // Property: WM_NAME = 39
        req.extend_from_slice(&31u32.to_le_bytes()); // Type: STRING = 31
        req.push(8); // Format: 8 bits per element
        req.extend_from_slice(&[0u8; 3]); // Padding
        req.extend_from_slice(&(title_bytes.len() as u32).to_le_bytes()); // Data length
        req.extend_from_slice(title_bytes);
        // Pad to 4-byte boundary
        while req.len() < 24 + padded_len {
            req.push(0);
        }

        self.send_request(&req)?;
        self.flush()?;

        if self.debug {
            log::debug!("Set window 0x{:x} title to: {}", server_wid, title);
        }
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
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()> {
        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolyRectangle request (opcode 67)
        let mut req = Vec::new();
        req.push(67); // Opcode: PolyRectangle
        req.push(0); // Padding
        req.extend_from_slice(&5u16.to_le_bytes()); // Length: 5 words
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
        req.extend_from_slice(&width.to_le_bytes());
        req.extend_from_slice(&height.to_le_bytes());

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Drew rectangle outline: drawable=0x{:x}, x={}, y={}, {}x{}",
                server_drawable,
                x,
                y,
                width,
                height
            );
        }

        Ok(())
    }

    fn fill_rectangle(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) -> BackendResult<()> {
        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => {
                let map = self.pixmap_map.lock().unwrap();
                log::debug!(
                    "fill_rectangle: looking up backend pixmap {}, map has {} entries: {:?}",
                    p,
                    map.len(),
                    map.keys().collect::<Vec<_>>()
                );
                *map.get(&p).ok_or_else(|| {
                    format!(
                        "Pixmap not found: backend_id={}, available={:?}",
                        p,
                        map.keys().collect::<Vec<_>>()
                    )
                })?
            }
        };

        // Create or get GC on server
        // For simplicity, create a temporary GC for each draw call
        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolyFillRectangle request (opcode 70)
        // Format: opcode(1), pad(1), length(2), drawable(4), gc(4), rectangles(8 each)
        // Length = 3 + 2*n words where n = number of rectangles
        let mut req = Vec::new();
        req.push(70); // Opcode: PolyFillRectangle
        req.push(0); // Padding
        req.extend_from_slice(&5u16.to_le_bytes()); // Length: 5 words = 20 bytes (1 rectangle)
        req.extend_from_slice(&server_drawable.to_le_bytes()); // drawable
        req.extend_from_slice(&gc_id.to_le_bytes()); // gc
        req.extend_from_slice(&x.to_le_bytes()); // x
        req.extend_from_slice(&y.to_le_bytes()); // y
        req.extend_from_slice(&width.to_le_bytes()); // width
        req.extend_from_slice(&height.to_le_bytes()); // height

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Filled rectangle: drawable=0x{:x}, gc=0x{:x}, fg=0x{:x}, x={}, y={}, {}x{}",
                server_drawable,
                gc_id,
                gc.foreground,
                x,
                y,
                width,
                height
            );
        }

        Ok(())
    }

    fn draw_line(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x1: i16,
        y1: i16,
        x2: i16,
        y2: i16,
    ) -> BackendResult<()> {
        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolySegment request (opcode 66) for a single line segment
        let mut req = Vec::new();
        req.push(66); // Opcode: PolySegment
        req.push(0); // Padding
        req.extend_from_slice(&5u16.to_le_bytes()); // Length: 5 words (header + 1 segment)
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.extend_from_slice(&x1.to_le_bytes());
        req.extend_from_slice(&y1.to_le_bytes());
        req.extend_from_slice(&x2.to_le_bytes());
        req.extend_from_slice(&y2.to_le_bytes());

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Drew line: drawable=0x{:x}, ({},{}) to ({},{})",
                server_drawable,
                x1,
                y1,
                x2,
                y2
            );
        }

        Ok(())
    }

    fn draw_points(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[Point],
    ) -> BackendResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolyPoint request (opcode 64)
        let mut req = Vec::new();
        req.push(64); // Opcode: PolyPoint
        req.push(0); // Coordinate mode: Origin
        let length = 3 + points.len() as u16; // header + points
        req.extend_from_slice(&length.to_le_bytes());
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());

        for point in points {
            req.extend_from_slice(&point.x.to_le_bytes());
            req.extend_from_slice(&point.y.to_le_bytes());
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Drew {} points: drawable=0x{:x}",
                points.len(),
                server_drawable
            );
        }

        Ok(())
    }

    fn draw_text(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        x: i16,
        y: i16,
        text: &str,
    ) -> BackendResult<()> {
        // Check if we have a font
        let font_id = match self.default_font_id {
            Some(id) => id,
            None => return Ok(()), // No font available, silently skip
        };

        if text.is_empty() {
            return Ok(());
        }

        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        // Create GC with font
        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc_with_font(server_drawable, gc_id, gc, Some(font_id))?;

        // Convert to bytes (Latin-1 encoding, truncate to 255 chars max per request)
        let text_bytes: Vec<u8> = text.chars().take(255).map(|c| c as u8).collect();
        let text_len = text_bytes.len();
        let text_pad = (4 - ((16 + text_len) % 4)) % 4;

        // Build ImageText8 request (opcode 76)
        // Format: opcode(1), n(1), length(2), drawable(4), gc(4), x(2), y(2), string
        let mut req = Vec::new();
        req.push(76); // Opcode: ImageText8
        req.push(text_len as u8); // n (string length)
        let length = 4 + (text_len + text_pad) / 4;
        req.extend_from_slice(&(length as u16).to_le_bytes());
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
        req.extend_from_slice(&text_bytes);

        // Pad to 4-byte boundary
        req.extend(std::iter::repeat_n(0u8, text_pad));

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Drew text: drawable=0x{:x}, ({},{}) \"{}\"",
                server_drawable,
                x,
                y,
                text
            );
        }

        Ok(())
    }

    fn draw_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[crate::protocol::Arc],
    ) -> BackendResult<()> {
        if arcs.is_empty() {
            return Ok(());
        }

        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolyArc request (opcode 68)
        // Each arc is 12 bytes (x, y, width, height, angle1, angle2)
        let mut req = Vec::new();
        req.push(68); // Opcode: PolyArc
        req.push(0); // Padding
        let length = 3 + (arcs.len() * 3) as u16; // header + arcs (each arc is 3 words = 12 bytes)
        req.extend_from_slice(&length.to_le_bytes());
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());

        for arc in arcs {
            req.extend_from_slice(&arc.x.to_le_bytes());
            req.extend_from_slice(&arc.y.to_le_bytes());
            req.extend_from_slice(&arc.width.to_le_bytes());
            req.extend_from_slice(&arc.height.to_le_bytes());
            req.extend_from_slice(&arc.angle1.to_le_bytes());
            req.extend_from_slice(&arc.angle2.to_le_bytes());
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!("Drew {} arcs: drawable=0x{:x}", arcs.len(), server_drawable);
        }

        Ok(())
    }

    fn fill_arcs(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        arcs: &[crate::protocol::Arc],
    ) -> BackendResult<()> {
        if arcs.is_empty() {
            return Ok(());
        }

        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PolyFillArc request (opcode 71)
        let mut req = Vec::new();
        req.push(71); // Opcode: PolyFillArc
        req.push(0); // Padding
        let length = 3 + (arcs.len() * 3) as u16;
        req.extend_from_slice(&length.to_le_bytes());
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());

        for arc in arcs {
            req.extend_from_slice(&arc.x.to_le_bytes());
            req.extend_from_slice(&arc.y.to_le_bytes());
            req.extend_from_slice(&arc.width.to_le_bytes());
            req.extend_from_slice(&arc.height.to_le_bytes());
            req.extend_from_slice(&arc.angle1.to_le_bytes());
            req.extend_from_slice(&arc.angle2.to_le_bytes());
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Filled {} arcs: drawable=0x{:x}",
                arcs.len(),
                server_drawable
            );
        }

        Ok(())
    }

    fn fill_polygon(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        points: &[crate::protocol::Point],
    ) -> BackendResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build FillPoly request (opcode 69)
        let mut req = Vec::new();
        req.push(69); // Opcode: FillPoly
        req.push(0); // Padding
        let length = 4 + points.len() as u16; // header (4 words) + points
        req.extend_from_slice(&length.to_le_bytes());
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.push(2); // shape = Convex
        req.push(0); // coordinate mode = Origin
        req.extend_from_slice(&[0, 0]); // padding

        for point in points {
            req.extend_from_slice(&point.x.to_le_bytes());
            req.extend_from_slice(&point.y.to_le_bytes());
        }

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Filled polygon with {} points: drawable=0x{:x}",
                points.len(),
                server_drawable
            );
        }

        Ok(())
    }

    fn copy_area(
        &mut self,
        src: BackendDrawable,
        dst: BackendDrawable,
        gc: &BackendGC,
        src_x: i16,
        src_y: i16,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
    ) -> BackendResult<()> {
        // Get server src drawable ID
        let server_src = match src {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Source window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Source pixmap not found")?,
        };

        // Get server dst drawable ID
        let server_dst = match dst {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Destination window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Destination pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_dst, gc_id, gc)?;

        // Build CopyArea request (opcode 62)
        // Format: opcode(1), pad(1), length(2), src(4), dst(4), gc(4),
        //         src_x(2), src_y(2), dst_x(2), dst_y(2), width(2), height(2)
        let mut req = Vec::new();
        req.push(62); // Opcode: CopyArea
        req.push(0); // Padding
        req.extend_from_slice(&7u16.to_le_bytes()); // Length: 7 words = 28 bytes
        req.extend_from_slice(&server_src.to_le_bytes());
        req.extend_from_slice(&server_dst.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.extend_from_slice(&src_x.to_le_bytes());
        req.extend_from_slice(&src_y.to_le_bytes());
        req.extend_from_slice(&dst_x.to_le_bytes());
        req.extend_from_slice(&dst_y.to_le_bytes());
        req.extend_from_slice(&width.to_le_bytes());
        req.extend_from_slice(&height.to_le_bytes());

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "CopyArea: src=0x{:x} ({},{}) -> dst=0x{:x} ({},{}), {}x{}",
                server_src,
                src_x,
                src_y,
                server_dst,
                dst_x,
                dst_y,
                width,
                height
            );
        }

        Ok(())
    }

    fn create_pixmap(&mut self, width: u16, height: u16, depth: u8) -> BackendResult<usize> {
        // Allocate IDs
        let our_id = self.next_resource_id;
        self.next_resource_id += 1;
        let server_pid = self.allocate_server_resource_id();

        // Get root window as the drawable reference
        let root_drawable = if let Some(ref setup) = self.setup_info {
            setup.roots[0].root.id().get()
        } else {
            return Err("Not initialized".into());
        };

        // Build CreatePixmap request (opcode 53)
        // Format: depth(1), opcode(1), length(2), pid(4), drawable(4), width(2), height(2)
        let mut req = vec![0u8; 16];
        req[0] = 53; // Opcode: CreatePixmap
        req[1] = depth; // depth in header byte
        req[2..4].copy_from_slice(&4u16.to_le_bytes()); // Length: 4 words
        req[4..8].copy_from_slice(&server_pid.to_le_bytes());
        req[8..12].copy_from_slice(&root_drawable.to_le_bytes());
        req[12..14].copy_from_slice(&width.to_le_bytes());
        req[14..16].copy_from_slice(&height.to_le_bytes());

        self.send_request(&req)?;

        // Store the mapping
        self.pixmap_map.lock().unwrap().insert(our_id, server_pid);

        if self.debug {
            log::debug!(
                "Created X11 pixmap: our_id={}, server_pid=0x{:x}, {}x{}, depth={}",
                our_id,
                server_pid,
                width,
                height,
                depth
            );
        }

        Ok(our_id)
    }

    fn free_pixmap(&mut self, pixmap: usize) -> BackendResult<()> {
        // Get and remove the server pixmap ID
        let server_pid = self
            .pixmap_map
            .lock()
            .unwrap()
            .remove(&pixmap)
            .ok_or("Pixmap not found")?;

        // Build FreePixmap request (opcode 54)
        let mut req = vec![0u8; 8];
        req[0] = 54; // Opcode: FreePixmap
        req[1] = 0; // unused
        req[2..4].copy_from_slice(&2u16.to_le_bytes()); // Length: 2 words
        req[4..8].copy_from_slice(&server_pid.to_le_bytes());

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "Freed X11 pixmap: our_id={}, server_pid=0x{:x}",
                pixmap,
                server_pid
            );
        }

        Ok(())
    }

    fn put_image(
        &mut self,
        drawable: BackendDrawable,
        gc: &BackendGC,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        data: &[u8],
    ) -> BackendResult<()> {
        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        let gc_id = self.allocate_server_resource_id();
        self.create_server_gc(server_drawable, gc_id, gc)?;

        // Build PutImage request (opcode 72)
        // Format: opcode(1), format(1), length(2), drawable(4), gc(4),
        //         width(2), height(2), dst_x(2), dst_y(2), left_pad(1), depth(1), pad(2), data
        let mut req = Vec::new();
        req.push(72); // Opcode: PutImage
        req.push(format); // format: 0=Bitmap, 1=XYPixmap, 2=ZPixmap

        // Calculate padded data length
        let data_pad = (4 - (data.len() % 4)) % 4;
        let total_len = 24 + data.len() + data_pad; // 24 byte header + data + padding
        let length_words = total_len / 4;
        req.extend_from_slice(&(length_words as u16).to_le_bytes());

        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&gc_id.to_le_bytes());
        req.extend_from_slice(&width.to_le_bytes());
        req.extend_from_slice(&height.to_le_bytes());
        req.extend_from_slice(&dst_x.to_le_bytes());
        req.extend_from_slice(&dst_y.to_le_bytes());
        req.push(0); // left_pad (usually 0 for ZPixmap)
        req.push(depth);
        req.extend_from_slice(&[0, 0]); // padding

        // Add image data
        req.extend_from_slice(data);

        // Pad to 4-byte boundary
        req.extend(std::iter::repeat_n(0u8, data_pad));

        self.send_request(&req)?;

        if self.debug {
            log::debug!(
                "PutImage: drawable=0x{:x}, {}x{} at ({},{}), depth={}, format={}, {} bytes",
                server_drawable,
                width,
                height,
                dst_x,
                dst_y,
                depth,
                format,
                data.len()
            );
        }

        Ok(())
    }

    fn get_image(
        &mut self,
        drawable: BackendDrawable,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
        format: u8,
    ) -> BackendResult<(u8, u32, Vec<u8>)> {
        // Get server drawable ID
        let server_drawable = match drawable {
            BackendDrawable::Window(w) => *self
                .window_map
                .lock()
                .unwrap()
                .get(&w.0)
                .ok_or("Window not found")?,
            BackendDrawable::Pixmap(p) => *self
                .pixmap_map
                .lock()
                .unwrap()
                .get(&p)
                .ok_or("Pixmap not found")?,
        };

        // Build GetImage request (opcode 73)
        // Format: opcode(1), format(1), length(2), drawable(4), x(2), y(2), width(2), height(2), plane_mask(4)
        let mut req = Vec::new();
        req.push(73); // Opcode: GetImage
        req.push(format); // format: 1=XYPixmap, 2=ZPixmap
        req.extend_from_slice(&5u16.to_le_bytes()); // Length: 5 words = 20 bytes
        req.extend_from_slice(&server_drawable.to_le_bytes());
        req.extend_from_slice(&x.to_le_bytes());
        req.extend_from_slice(&y.to_le_bytes());
        req.extend_from_slice(&width.to_le_bytes());
        req.extend_from_slice(&height.to_le_bytes());
        req.extend_from_slice(&plane_mask.to_le_bytes());

        // Send request and get reply
        let reply = self.send_request_with_reply(&req)?;

        // GetImage reply format:
        // header[0] = 1 (reply type)
        // header[1] = depth
        // header[2..4] = sequence
        // header[4..8] = length (additional data in 4-byte units)
        // header[8..12] = visual
        // header[12..32] = padding
        // data follows header

        if reply.len() < 32 {
            return Err("GetImage reply too short".into());
        }

        let depth = reply[1];
        let visual = u32::from_le_bytes([reply[8], reply[9], reply[10], reply[11]]);

        // Extract image data (skip 32-byte header)
        let image_data = reply[32..].to_vec();

        if self.debug {
            log::debug!(
                "GetImage: drawable=0x{:x}, ({},{}) {}x{}, depth={}, visual=0x{:x}, {} bytes",
                server_drawable,
                x,
                y,
                width,
                height,
                depth,
                visual,
                image_data.len()
            );
        }

        Ok((depth, visual, image_data))
    }

    fn poll_events(&mut self) -> BackendResult<Vec<BackendEvent>> {
        Ok(Vec::new())
    }

    fn flush(&mut self) -> BackendResult<()> {
        if let Some(stream) = &mut self.connection {
            stream.flush().map_err(|e| format!("Flush failed: {}", e))?;
        }
        Ok(())
    }

    fn wait_for_event(&mut self) -> BackendResult<BackendEvent> {
        // This would need to read events from the X server
        // For now, return an error
        Err("wait_for_event not implemented for X11 backend".into())
    }

    fn list_system_fonts(&mut self) -> BackendResult<Vec<BackendFontInfo>> {
        if self.connection.is_none() {
            return Ok(Vec::new());
        }

        // Build ListFonts request (opcode 49)
        // Format: opcode(1), unused(1), length(2), max_names(2), pattern_length(2), pattern(...)
        let pattern = "-*-*-*-*-*-*-*-*-*-*-*-*-*-*";
        let pattern_bytes = pattern.as_bytes();
        let padded_len = (pattern_bytes.len() + 3) & !3;
        let total_len = (8 + padded_len) / 4;

        let mut req = Vec::new();
        req.push(49); // Opcode: ListFonts
        req.push(0); // Unused
        req.extend_from_slice(&(total_len as u16).to_le_bytes()); // Length
        req.extend_from_slice(&1000u16.to_le_bytes()); // max-names: up to 1000 fonts
        req.extend_from_slice(&(pattern_bytes.len() as u16).to_le_bytes()); // pattern length
        req.extend_from_slice(pattern_bytes);
        // Pad to 4-byte boundary
        while req.len() < 8 + padded_len {
            req.push(0);
        }

        let reply = self.send_request_with_reply(&req)?;

        // Parse ListFonts reply
        // Format: reply(1), unused(1), sequence(2), length(4), num_fonts(2), unused(22), names...
        if reply.len() < 32 {
            return Err("Invalid ListFonts reply".into());
        }

        let num_fonts = u16::from_le_bytes([reply[8], reply[9]]) as usize;

        if self.debug {
            log::debug!("X11 server returned {} fonts", num_fonts);
        }

        let mut fonts = Vec::new();
        let mut offset = 32; // Start after header

        for _ in 0..num_fonts {
            if offset >= reply.len() {
                break;
            }

            // Each name is: length(1), name(length)
            let name_len = reply[offset] as usize;
            offset += 1;

            if offset + name_len > reply.len() {
                break;
            }

            if let Ok(font_name) = std::str::from_utf8(&reply[offset..offset + name_len]) {
                if let Some(font_info) = Self::parse_xlfd(font_name) {
                    fonts.push(font_info);
                }
            }
            offset += name_len;
        }

        if self.debug {
            log::debug!("Parsed {} valid XLFD fonts from X server", fonts.len());
        }

        Ok(fonts)
    }
}

impl X11Backend {
    /// Get the parsed setup info from the real X server
    /// This is useful for debugging - we can return this to clients
    pub fn setup_info(&self) -> Option<&SetupSuccess> {
        self.setup_info.as_ref()
    }

    /// Parse an XLFD font name into BackendFontInfo
    fn parse_xlfd(xlfd: &str) -> Option<BackendFontInfo> {
        // XLFD format:
        // -foundry-family-weight-slant-setwidth-addstyle-pixel-point-resx-resy-spacing-avgwidth-registry-encoding
        let parts: Vec<&str> = xlfd.split('-').collect();
        if parts.len() < 15 {
            return None;
        }

        // parts[0] is empty (string starts with -)
        // parts[1] = foundry
        // parts[2] = family
        // parts[3] = weight
        // parts[4] = slant
        // parts[5] = setwidth
        // parts[6] = addstyle
        // parts[7] = pixel size
        // parts[8] = point size (decipoints)
        // parts[9] = resx
        // parts[10] = resy
        // parts[11] = spacing
        // parts[12] = avgwidth
        // parts[13] = registry
        // parts[14] = encoding

        let family = parts[2].to_string();
        let weight = parts[3].to_string();
        let slant = parts[4].to_string();

        let pixel_size = parts[7].parse().unwrap_or(0);
        let point_size = parts[8].parse().unwrap_or(0);
        let char_width = parts[12].parse().unwrap_or(0);

        let registry = parts[13].to_string();
        let encoding = parts[14].to_string();

        // For ascent/descent, we'd need to query font properties
        // Use reasonable defaults based on pixel size
        let ascent = if pixel_size > 0 {
            pixel_size as i16 * 3 / 4
        } else {
            10
        };
        let descent = if pixel_size > 0 {
            pixel_size as i16 / 4
        } else {
            3
        };

        Some(BackendFontInfo {
            xlfd_name: xlfd.to_string(),
            family,
            weight,
            slant,
            pixel_size,
            point_size,
            char_width,
            ascent,
            descent,
            registry,
            encoding,
        })
    }

    /// Get a cloned connection to the real X server for direct passthrough
    /// This is useful for bidirectional proxying
    pub fn clone_connection(&self) -> BackendResult<UnixStream> {
        match &self.connection {
            Some(stream) => stream
                .try_clone()
                .map_err(|e| format!("Failed to clone connection: {}", e).into()),
            None => Err("Backend not initialized".into()),
        }
    }
}
