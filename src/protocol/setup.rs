//! X11 connection setup protocol
//!
//! This module handles the initial connection handshake between client and server.

use super::*;
use std::io::{Read, Write};

/// Connection setup request from client
#[derive(Debug, Clone)]
pub struct SetupRequest {
    pub byte_order: ByteOrder,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub authorization_protocol_name: String,
    pub authorization_protocol_data: Vec<u8>,
}

impl SetupRequest {
    /// Parse setup request from stream
    pub fn parse<R: Read>(stream: &mut R) -> Result<Self, X11Error> {
        let mut header = [0u8; 12];
        stream.read_exact(&mut header)
            .map_err(|_| X11Error::bad_length(0, 0))?;

        // Byte 0: byte order ('B' = MSB, 'l' = LSB)
        let byte_order = match header[0] {
            b'B' => ByteOrder::MSBFirst,
            b'l' => ByteOrder::LSBFirst,
            _ => return Err(X11Error::bad_request(0, 0)),
        };

        // Helper to read u16 with correct byte order
        let read_u16 = |bytes: &[u8]| -> u16 {
            match byte_order {
                ByteOrder::MSBFirst => u16::from_be_bytes([bytes[0], bytes[1]]),
                ByteOrder::LSBFirst => u16::from_le_bytes([bytes[0], bytes[1]]),
            }
        };

        let protocol_major_version = read_u16(&header[2..4]);
        let protocol_minor_version = read_u16(&header[4..6]);
        let auth_proto_name_len = read_u16(&header[6..8]) as usize;
        let auth_proto_data_len = read_u16(&header[8..10]) as usize;

        // Read authorization protocol name (padded to 4 bytes)
        let auth_name_padded = padded_len(auth_proto_name_len);
        let mut auth_name_buf = vec![0u8; auth_name_padded];
        stream.read_exact(&mut auth_name_buf)
            .map_err(|_| X11Error::bad_length(0, 0))?;
        let authorization_protocol_name = String::from_utf8_lossy(&auth_name_buf[..auth_proto_name_len])
            .to_string();

        // Read authorization protocol data (padded to 4 bytes)
        let auth_data_padded = padded_len(auth_proto_data_len);
        let mut authorization_protocol_data = vec![0u8; auth_data_padded];
        stream.read_exact(&mut authorization_protocol_data)
            .map_err(|_| X11Error::bad_length(0, 0))?;
        authorization_protocol_data.truncate(auth_proto_data_len);

        Ok(SetupRequest {
            byte_order,
            protocol_major_version,
            protocol_minor_version,
            authorization_protocol_name,
            authorization_protocol_data,
        })
    }
}

/// Setup response status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStatus {
    Failed = 0,
    Success = 1,
    Authenticate = 2,
}

/// Format information
#[derive(Debug, Clone)]
pub struct Format {
    pub depth: u8,
    pub bits_per_pixel: u8,
    pub scanline_pad: u8,
}

impl Format {
    pub fn encode(&self, buffer: &mut Vec<u8>) {
        buffer.push(self.depth);
        buffer.push(self.bits_per_pixel);
        buffer.push(self.scanline_pad);
        buffer.extend_from_slice(&[0u8; 5]); // Padding
    }
}

/// Visual type information
#[derive(Debug, Clone)]
pub struct VisualType {
    pub visual_id: VisualID,
    pub class: u8,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}

impl VisualType {
    pub fn encode(&self, buffer: &mut Vec<u8>, byte_order: ByteOrder) {
        let write_u16 = |buf: &mut Vec<u8>, val: u16| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        let write_u32 = |buf: &mut Vec<u8>, val: u32| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        write_u32(buffer, self.visual_id.get());
        buffer.push(self.class);
        buffer.push(self.bits_per_rgb_value);
        write_u16(buffer, self.colormap_entries);
        write_u32(buffer, self.red_mask);
        write_u32(buffer, self.green_mask);
        write_u32(buffer, self.blue_mask);
        buffer.extend_from_slice(&[0u8; 4]); // Padding
    }
}

/// Depth information
#[derive(Debug, Clone)]
pub struct Depth {
    pub depth: u8,
    pub visuals: Vec<VisualType>,
}

impl Depth {
    pub fn encode(&self, buffer: &mut Vec<u8>, byte_order: ByteOrder) {
        let write_u16 = |buf: &mut Vec<u8>, val: u16| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        buffer.push(self.depth);
        buffer.push(0); // Padding
        write_u16(buffer, self.visuals.len() as u16);
        buffer.extend_from_slice(&[0u8; 4]); // Padding

        for visual in &self.visuals {
            visual.encode(buffer, byte_order);
        }
    }
}

/// Screen information
#[derive(Debug, Clone)]
pub struct Screen {
    pub root: Window,
    pub default_colormap: Colormap,
    pub white_pixel: u32,
    pub black_pixel: u32,
    pub current_input_masks: u32,
    pub width_in_pixels: u16,
    pub height_in_pixels: u16,
    pub width_in_millimeters: u16,
    pub height_in_millimeters: u16,
    pub min_installed_maps: u16,
    pub max_installed_maps: u16,
    pub root_visual: VisualID,
    pub backing_stores: u8,
    pub save_unders: bool,
    pub root_depth: u8,
    pub allowed_depths: Vec<Depth>,
}

impl Screen {
    pub fn encode(&self, buffer: &mut Vec<u8>, byte_order: ByteOrder) {
        let write_u16 = |buf: &mut Vec<u8>, val: u16| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        let write_u32 = |buf: &mut Vec<u8>, val: u32| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        write_u32(buffer, self.root.id().get());
        write_u32(buffer, self.default_colormap.id().get());
        write_u32(buffer, self.white_pixel);
        write_u32(buffer, self.black_pixel);
        write_u32(buffer, self.current_input_masks);
        write_u16(buffer, self.width_in_pixels);
        write_u16(buffer, self.height_in_pixels);
        write_u16(buffer, self.width_in_millimeters);
        write_u16(buffer, self.height_in_millimeters);
        write_u16(buffer, self.min_installed_maps);
        write_u16(buffer, self.max_installed_maps);
        write_u32(buffer, self.root_visual.get());
        buffer.push(self.backing_stores);
        buffer.push(if self.save_unders { 1 } else { 0 });
        buffer.push(self.root_depth);
        buffer.push(self.allowed_depths.len() as u8);

        for depth in &self.allowed_depths {
            depth.encode(buffer, byte_order);
        }
    }
}

/// Setup reply (success case)
#[derive(Debug, Clone)]
pub struct SetupSuccess {
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub release_number: u32,
    pub resource_id_base: u32,
    pub resource_id_mask: u32,
    pub motion_buffer_size: u32,
    pub maximum_request_length: u16,
    pub image_byte_order: ByteOrder,
    pub bitmap_format_bit_order: ByteOrder,
    pub bitmap_format_scanline_unit: u8,
    pub bitmap_format_scanline_pad: u8,
    pub min_keycode: u8,
    pub max_keycode: u8,
    pub vendor: String,
    pub pixmap_formats: Vec<Format>,
    pub roots: Vec<Screen>,
}

impl SetupSuccess {
    pub fn encode<W: Write>(&self, stream: &mut W, byte_order: ByteOrder) -> std::io::Result<()> {
        let mut buffer = Vec::new();

        let write_u16 = |buf: &mut Vec<u8>, val: u16| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        let write_u32 = |buf: &mut Vec<u8>, val: u32| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        // Status (1 = Success)
        buffer.push(SetupStatus::Success as u8);

        // Unused byte
        buffer.push(0);

        // Protocol version
        write_u16(&mut buffer, self.protocol_major_version);
        write_u16(&mut buffer, self.protocol_minor_version);

        // We'll come back and fill in the length
        let length_pos = buffer.len();
        write_u16(&mut buffer, 0); // Placeholder for length

        // Release number
        write_u32(&mut buffer, self.release_number);
        write_u32(&mut buffer, self.resource_id_base);
        write_u32(&mut buffer, self.resource_id_mask);
        write_u32(&mut buffer, self.motion_buffer_size);

        // Vendor length
        write_u16(&mut buffer, self.vendor.len() as u16);

        // Maximum request length
        write_u16(&mut buffer, self.maximum_request_length);

        // Number of screens and formats
        buffer.push(self.roots.len() as u8);
        buffer.push(self.pixmap_formats.len() as u8);

        // Image byte order
        buffer.push(self.image_byte_order as u8);
        buffer.push(self.bitmap_format_bit_order as u8);
        buffer.push(self.bitmap_format_scanline_unit);
        buffer.push(self.bitmap_format_scanline_pad);
        buffer.push(self.min_keycode);
        buffer.push(self.max_keycode);

        // Padding
        buffer.extend_from_slice(&[0u8; 4]);

        // Vendor string (padded to 4 bytes)
        buffer.extend_from_slice(self.vendor.as_bytes());
        let vendor_pad = pad(self.vendor.len());
        buffer.extend_from_slice(&vec![0u8; vendor_pad]);

        // Pixmap formats
        for format in &self.pixmap_formats {
            format.encode(&mut buffer);
        }

        // Screens
        for screen in &self.roots {
            screen.encode(&mut buffer, byte_order);
        }

        // Calculate and write length (in 4-byte units, excluding first 8 bytes)
        let additional_bytes = buffer.len() - 8;
        let length = (additional_bytes / 4) as u16;

        match byte_order {
            ByteOrder::MSBFirst => {
                buffer[length_pos..length_pos + 2].copy_from_slice(&length.to_be_bytes());
            }
            ByteOrder::LSBFirst => {
                buffer[length_pos..length_pos + 2].copy_from_slice(&length.to_le_bytes());
            }
        }

        stream.write_all(&buffer)
    }
}

/// Setup failed response
#[derive(Debug, Clone)]
pub struct SetupFailed {
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub reason: String,
}

impl SetupFailed {
    pub fn encode<W: Write>(&self, stream: &mut W, byte_order: ByteOrder) -> std::io::Result<()> {
        let mut buffer = Vec::new();

        let write_u16 = |buf: &mut Vec<u8>, val: u16| {
            match byte_order {
                ByteOrder::MSBFirst => buf.extend_from_slice(&val.to_be_bytes()),
                ByteOrder::LSBFirst => buf.extend_from_slice(&val.to_le_bytes()),
            }
        };

        // Status (0 = Failed)
        buffer.push(SetupStatus::Failed as u8);

        // Reason length
        buffer.push(self.reason.len() as u8);

        // Protocol version
        write_u16(&mut buffer, self.protocol_major_version);
        write_u16(&mut buffer, self.protocol_minor_version);

        // Length in 4-byte units
        let reason_padded = padded_len(self.reason.len());
        let length = (reason_padded / 4) as u16;
        write_u16(&mut buffer, length);

        // Reason string (padded to 4 bytes)
        buffer.extend_from_slice(self.reason.as_bytes());
        let reason_pad = pad(self.reason.len());
        buffer.extend_from_slice(&vec![0u8; reason_pad]);

        stream.write_all(&buffer)
    }
}

/// Setup response
#[derive(Debug, Clone)]
pub enum SetupResponse {
    Success(SetupSuccess),
    Failed(SetupFailed),
}

impl SetupResponse {
    pub fn encode<W: Write>(&self, stream: &mut W, byte_order: ByteOrder) -> std::io::Result<()> {
        match self {
            SetupResponse::Success(success) => success.encode(stream, byte_order),
            SetupResponse::Failed(failed) => failed.encode(stream, byte_order),
        }
    }
}
