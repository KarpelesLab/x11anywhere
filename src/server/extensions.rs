//! X11 Extension handlers
//!
//! This module handles requests for X11 extensions like COMPOSITE, XFIXES, DAMAGE, etc.

use std::io::Write;
use std::net::TcpStream;

/// Handle extension request based on major opcode
pub fn handle_extension_request(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    major_opcode: u8,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let minor_opcode = header[1];
    let sequence = u16::from_le_bytes([header[2], header[3]]);

    log::debug!(
        "Extension request: major={}, minor={}, seq={}",
        major_opcode,
        minor_opcode,
        sequence
    );

    match major_opcode {
        129 => handle_shape_request(stream, minor_opcode, sequence, data),
        130 => handle_shm_request(stream, minor_opcode, sequence, data),
        133 => handle_big_requests(stream, minor_opcode, sequence, data),
        134 => handle_sync_request(stream, minor_opcode, sequence, data),
        138 => handle_xfixes_request(stream, minor_opcode, sequence, data),
        139 => handle_render_request(stream, minor_opcode, sequence, data),
        140 => handle_randr_request(stream, minor_opcode, sequence, data),
        142 => handle_composite_request(stream, minor_opcode, sequence, data),
        143 => handle_damage_request(stream, minor_opcode, sequence, data),
        _ => {
            log::debug!("Unknown extension major opcode: {}", major_opcode);
            Ok(())
        }
    }
}

/// Handle SHAPE extension requests
fn handle_shape_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // ShapeQueryVersion
            log::debug!("SHAPE: QueryVersion");
            let reply = encode_shape_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("SHAPE: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle MIT-SHM extension requests
fn handle_shm_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // ShmQueryVersion
            log::debug!("MIT-SHM: QueryVersion");
            let reply = encode_shm_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("MIT-SHM: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle BIG-REQUESTS extension
fn handle_big_requests(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // BigReqEnable
            log::debug!("BIG-REQUESTS: Enable");
            let reply = encode_big_requests_enable_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("BIG-REQUESTS: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle SYNC extension requests
fn handle_sync_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // SyncInitialize
            log::debug!("SYNC: Initialize");
            let reply = encode_sync_initialize_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("SYNC: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle XFIXES extension requests
fn handle_xfixes_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // XFixesQueryVersion
            log::debug!("XFIXES: QueryVersion");
            let reply = encode_xfixes_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("XFIXES: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle RENDER extension requests
fn handle_render_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // RenderQueryVersion
            log::debug!("RENDER: QueryVersion");
            let reply = encode_render_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("RENDER: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle RANDR extension requests
fn handle_randr_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    _data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // RRQueryVersion
            log::debug!("RANDR: QueryVersion");
            let reply = encode_randr_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("RANDR: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle COMPOSITE extension requests
fn handle_composite_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // CompositeQueryVersion
            log::debug!("COMPOSITE: QueryVersion");
            let reply = encode_composite_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        1 => {
            // CompositeRedirectWindow
            if data.len() >= 5 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let update = data[4];
                log::debug!(
                    "COMPOSITE: RedirectWindow window=0x{:x} update={}",
                    window,
                    update
                );
            }
            // No reply for RedirectWindow
        }
        2 => {
            // CompositeRedirectSubwindows
            if data.len() >= 5 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let update = data[4];
                log::debug!(
                    "COMPOSITE: RedirectSubwindows window=0x{:x} update={}",
                    window,
                    update
                );
            }
            // No reply
        }
        3 => {
            // CompositeUnredirectWindow
            if data.len() >= 5 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let update = data[4];
                log::debug!(
                    "COMPOSITE: UnredirectWindow window=0x{:x} update={}",
                    window,
                    update
                );
            }
            // No reply
        }
        4 => {
            // CompositeUnredirectSubwindows
            if data.len() >= 5 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let update = data[4];
                log::debug!(
                    "COMPOSITE: UnredirectSubwindows window=0x{:x} update={}",
                    window,
                    update
                );
            }
            // No reply
        }
        6 => {
            // CompositeNameWindowPixmap
            if data.len() >= 8 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let pixmap = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "COMPOSITE: NameWindowPixmap window=0x{:x} pixmap=0x{:x}",
                    window,
                    pixmap
                );
            }
            // No reply - creates a pixmap
        }
        7 => {
            // CompositeGetOverlayWindow
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("COMPOSITE: GetOverlayWindow window=0x{:x}", window);
                // Return the root window as overlay for now
                let reply = encode_composite_get_overlay_window_reply(sequence, window);
                stream.write_all(&reply)?;
            }
        }
        8 => {
            // CompositeReleaseOverlayWindow
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("COMPOSITE: ReleaseOverlayWindow window=0x{:x}", window);
            }
            // No reply
        }
        _ => {
            log::debug!("COMPOSITE: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

/// Handle DAMAGE extension requests
fn handle_damage_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // DamageQueryVersion
            log::debug!("DAMAGE: QueryVersion");
            let reply = encode_damage_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        1 => {
            // DamageCreate
            if data.len() >= 9 {
                let damage = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let drawable = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let level = data[8];
                log::debug!(
                    "DAMAGE: Create damage=0x{:x} drawable=0x{:x} level={}",
                    damage,
                    drawable,
                    level
                );
            }
            // No reply
        }
        2 => {
            // DamageDestroy
            if data.len() >= 4 {
                let damage = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("DAMAGE: Destroy damage=0x{:x}", damage);
            }
            // No reply
        }
        3 => {
            // DamageSubtract
            if data.len() >= 12 {
                let damage = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let repair = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let parts = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "DAMAGE: Subtract damage=0x{:x} repair=0x{:x} parts=0x{:x}",
                    damage,
                    repair,
                    parts
                );
            }
            // No reply
        }
        _ => {
            log::debug!("DAMAGE: Unhandled minor opcode {}", minor_opcode);
        }
    }
    Ok(())
}

// Reply encoders

fn write_u16_le(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

fn write_u32_le(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

fn encode_shape_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(1)); // major version
    buffer[10..12].copy_from_slice(&write_u16_le(1)); // minor version
    buffer
}

fn encode_shm_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = 1; // shared pixmaps supported
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(1)); // major version
    buffer[10..12].copy_from_slice(&write_u16_le(2)); // minor version
    buffer[12..14].copy_from_slice(&write_u16_le(0)); // uid (not used)
    buffer[14..16].copy_from_slice(&write_u16_le(0)); // gid (not used)
    buffer[16] = 0; // pixmap format
    buffer
}

fn encode_big_requests_enable_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
                                                    // Maximum request length in 4-byte units (4MB)
    buffer[8..12].copy_from_slice(&write_u32_le(0x100000));
    buffer
}

fn encode_sync_initialize_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8] = 3; // major version
    buffer[9] = 1; // minor version
    buffer
}

fn encode_xfixes_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(5)); // major version
    buffer[12..16].copy_from_slice(&write_u32_le(0)); // minor version
    buffer
}

fn encode_render_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // major version
    buffer[12..16].copy_from_slice(&write_u32_le(11)); // minor version
    buffer
}

fn encode_randr_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(1)); // major version
    buffer[12..16].copy_from_slice(&write_u32_le(5)); // minor version
    buffer
}

fn encode_composite_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // major version
    buffer[12..16].copy_from_slice(&write_u32_le(4)); // minor version
    buffer
}

fn encode_composite_get_overlay_window_reply(sequence: u16, overlay_window: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(overlay_window)); // overlay window
    buffer
}

fn encode_damage_query_version_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(1)); // major version
    buffer[12..16].copy_from_slice(&write_u32_le(1)); // minor version
    buffer
}
