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

    // For generating our own resource IDs
    next_resource_id: usize,
    resource_id_base: u32,
    resource_id_mask: u32,

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
            next_resource_id: 1,
            resource_id_base: 0,
            resource_id_mask: 0,
            debug: true,
        }
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    fn connect_to_display(&mut self) -> BackendResult<()> {
        // Parse display number
        let display_num: usize = self
            .display
            .trim_start_matches(':')
            .parse()
            .map_err(|_| "Invalid display number")?;

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

    fn create_window(&mut self, _params: WindowParams) -> BackendResult<BackendWindow> {
        // For passthrough, just return a handle
        // In a real implementation, we'd send CreateWindow request to the real server
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        Ok(BackendWindow(id))
    }

    fn destroy_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn map_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
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

    fn raise_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn lower_window(&mut self, _window: BackendWindow) -> BackendResult<()> {
        Ok(())
    }

    fn set_window_title(&mut self, _window: BackendWindow, _title: &str) -> BackendResult<()> {
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
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _width: u16,
        _height: u16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn fill_rectangle(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _width: u16,
        _height: u16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_line(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x1: i16,
        _y1: i16,
        _x2: i16,
        _y2: i16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_points(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _points: &[Point],
    ) -> BackendResult<()> {
        Ok(())
    }

    fn draw_text(
        &mut self,
        _drawable: BackendDrawable,
        _gc: &BackendGC,
        _x: i16,
        _y: i16,
        _text: &str,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn copy_area(
        &mut self,
        _src: BackendDrawable,
        _dst: BackendDrawable,
        _gc: &BackendGC,
        _src_x: i16,
        _src_y: i16,
        _width: u16,
        _height: u16,
        _dst_x: i16,
        _dst_y: i16,
    ) -> BackendResult<()> {
        Ok(())
    }

    fn create_pixmap(&mut self, _width: u16, _height: u16, _depth: u8) -> BackendResult<usize> {
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        Ok(id)
    }

    fn free_pixmap(&mut self, _pixmap: usize) -> BackendResult<()> {
        Ok(())
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
}

impl X11Backend {
    /// Get the parsed setup info from the real X server
    /// This is useful for debugging - we can return this to clients
    pub fn setup_info(&self) -> Option<&SetupSuccess> {
        self.setup_info.as_ref()
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
