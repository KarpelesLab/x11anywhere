//! X11 Extension handlers
//!
//! This module handles requests for X11 extensions like COMPOSITE, XFIXES, DAMAGE, etc.

use super::Server;
use crate::backend::RenderTrapezoid;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// Handle extension request based on major opcode
pub fn handle_extension_request(
    stream: &mut TcpStream,
    header: &[u8],
    data: &[u8],
    major_opcode: u8,
    server: &Arc<Mutex<Server>>,
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
        135 => handle_xkb_request(stream, minor_opcode, sequence, data),
        138 => handle_xfixes_request(stream, minor_opcode, sequence, data),
        139 => handle_render_request(stream, minor_opcode, sequence, data, server),
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
    data: &[u8],
    server: &Arc<Mutex<Server>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // RenderQueryVersion
            log::debug!("RENDER: QueryVersion");
            let reply = encode_render_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        1 => {
            // RenderQueryPictFormats
            log::debug!("RENDER: QueryPictFormats");
            let reply = encode_render_query_pict_formats_reply(sequence);
            stream.write_all(&reply)?;
        }
        4 => {
            // RenderCreatePicture
            // Format: picture(4) + drawable(4) + format(4) + value_mask(4) + values...
            if data.len() >= 16 {
                let picture_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let drawable = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let format = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "RENDER: CreatePicture picture=0x{:x} drawable=0x{:x} format={}",
                    picture_id,
                    drawable,
                    format
                );
                let mut server = server.lock().unwrap();
                server.create_picture(picture_id, drawable, format);
            }
        }
        5 => {
            // RenderChangePicture - no reply needed
            log::debug!("RENDER: ChangePicture");
        }
        6 => {
            // RenderSetPictureClipRectangles - no reply needed
            log::debug!("RENDER: SetPictureClipRectangles");
        }
        7 => {
            // RenderFreePicture
            if data.len() >= 4 {
                let picture_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RENDER: FreePicture picture=0x{:x}", picture_id);
                let mut server = server.lock().unwrap();
                server.free_picture(picture_id);
            }
        }
        8 => {
            // RenderComposite - no reply needed
            log::debug!("RENDER: Composite");
        }
        10 => {
            // RenderTrapezoids
            // Format: op(1) + unused(3) + src(4) + dst(4) + mask_format(4) + src_x(2) + src_y(2) + trapezoids...
            if data.len() >= 24 {
                let op = data[0];
                let src_picture = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let dst_picture = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let mask_format = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                let src_x = i16::from_le_bytes([data[16], data[17]]);
                let src_y = i16::from_le_bytes([data[18], data[19]]);

                // Each trapezoid is 40 bytes (10 * 4-byte fixed-point values)
                let trap_data = &data[20..];
                let num_trapezoids = trap_data.len() / 40;

                log::debug!(
                    "RENDER: Trapezoids op={} src=0x{:x} dst=0x{:x} mask_format={} src=({},{}) count={}",
                    op,
                    src_picture,
                    dst_picture,
                    mask_format,
                    src_x,
                    src_y,
                    num_trapezoids
                );

                let mut trapezoids = Vec::with_capacity(num_trapezoids);
                for i in 0..num_trapezoids {
                    let offset = i * 40;
                    let trap = RenderTrapezoid {
                        top: i32::from_le_bytes([
                            trap_data[offset],
                            trap_data[offset + 1],
                            trap_data[offset + 2],
                            trap_data[offset + 3],
                        ]),
                        bottom: i32::from_le_bytes([
                            trap_data[offset + 4],
                            trap_data[offset + 5],
                            trap_data[offset + 6],
                            trap_data[offset + 7],
                        ]),
                        left_x1: i32::from_le_bytes([
                            trap_data[offset + 8],
                            trap_data[offset + 9],
                            trap_data[offset + 10],
                            trap_data[offset + 11],
                        ]),
                        left_y1: i32::from_le_bytes([
                            trap_data[offset + 12],
                            trap_data[offset + 13],
                            trap_data[offset + 14],
                            trap_data[offset + 15],
                        ]),
                        left_x2: i32::from_le_bytes([
                            trap_data[offset + 16],
                            trap_data[offset + 17],
                            trap_data[offset + 18],
                            trap_data[offset + 19],
                        ]),
                        left_y2: i32::from_le_bytes([
                            trap_data[offset + 20],
                            trap_data[offset + 21],
                            trap_data[offset + 22],
                            trap_data[offset + 23],
                        ]),
                        right_x1: i32::from_le_bytes([
                            trap_data[offset + 24],
                            trap_data[offset + 25],
                            trap_data[offset + 26],
                            trap_data[offset + 27],
                        ]),
                        right_y1: i32::from_le_bytes([
                            trap_data[offset + 28],
                            trap_data[offset + 29],
                            trap_data[offset + 30],
                            trap_data[offset + 31],
                        ]),
                        right_x2: i32::from_le_bytes([
                            trap_data[offset + 32],
                            trap_data[offset + 33],
                            trap_data[offset + 34],
                            trap_data[offset + 35],
                        ]),
                        right_y2: i32::from_le_bytes([
                            trap_data[offset + 36],
                            trap_data[offset + 37],
                            trap_data[offset + 38],
                            trap_data[offset + 39],
                        ]),
                    };
                    trapezoids.push(trap);
                }

                let mut server = server.lock().unwrap();
                if let Err(e) = server.render_trapezoids(
                    op,
                    src_picture,
                    dst_picture,
                    mask_format,
                    src_x,
                    src_y,
                    &trapezoids,
                ) {
                    log::warn!("RENDER: Trapezoids error: {}", e);
                }
            }
        }
        11 => {
            // RenderTriangles - no reply needed
            log::debug!("RENDER: Triangles");
        }
        17 => {
            // RenderCreateGlyphSet - no reply needed
            log::debug!("RENDER: CreateGlyphSet");
        }
        18 => {
            // RenderReferenceGlyphSet - no reply needed
            log::debug!("RENDER: ReferenceGlyphSet");
        }
        19 => {
            // RenderFreeGlyphSet - no reply needed
            log::debug!("RENDER: FreeGlyphSet");
        }
        20 => {
            // RenderAddGlyphs - no reply needed
            log::debug!("RENDER: AddGlyphs");
        }
        23 => {
            // RenderCompositeGlyphs8 - no reply needed
            log::debug!("RENDER: CompositeGlyphs8");
        }
        24 => {
            // RenderCompositeGlyphs16 - no reply needed
            log::debug!("RENDER: CompositeGlyphs16");
        }
        25 => {
            // RenderCompositeGlyphs32 - no reply needed
            log::debug!("RENDER: CompositeGlyphs32");
        }
        26 => {
            // RenderFillRectangles - no reply needed
            log::debug!("RENDER: FillRectangles");
        }
        27 => {
            // RenderCreateCursor - no reply needed
            log::debug!("RENDER: CreateCursor");
        }
        28 => {
            // RenderSetPictureTransform - no reply needed
            log::debug!("RENDER: SetPictureTransform");
        }
        29 => {
            // RenderQueryFilters
            log::debug!("RENDER: QueryFilters");
            let reply = encode_render_query_filters_reply(sequence);
            stream.write_all(&reply)?;
        }
        30 => {
            // RenderSetPictureFilter - no reply needed
            log::debug!("RENDER: SetPictureFilter");
        }
        31 => {
            // RenderCreateAnimCursor - no reply needed
            log::debug!("RENDER: CreateAnimCursor");
        }
        32 => {
            // RenderAddTraps - no reply needed
            log::debug!("RENDER: AddTraps");
        }
        33 => {
            // RenderCreateSolidFill
            // Format: picture(4) + color(8: red(2) + green(2) + blue(2) + alpha(2))
            if data.len() >= 12 {
                let picture_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let red = u16::from_le_bytes([data[4], data[5]]);
                let green = u16::from_le_bytes([data[6], data[7]]);
                let blue = u16::from_le_bytes([data[8], data[9]]);
                let alpha = u16::from_le_bytes([data[10], data[11]]);
                log::debug!(
                    "RENDER: CreateSolidFill picture=0x{:x} rgba({},{},{},{})",
                    picture_id,
                    red,
                    green,
                    blue,
                    alpha
                );
                let mut server = server.lock().unwrap();
                server.create_solid_fill(picture_id, red, green, blue, alpha);
            }
        }
        34 => {
            // RenderCreateLinearGradient - no reply needed
            log::debug!("RENDER: CreateLinearGradient");
        }
        35 => {
            // RenderCreateRadialGradient - no reply needed
            log::debug!("RENDER: CreateRadialGradient");
        }
        36 => {
            // RenderCreateConicalGradient - no reply needed
            log::debug!("RENDER: CreateConicalGradient");
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

/// Handle XKEYBOARD (XKB) extension requests
fn handle_xkb_request(
    stream: &mut TcpStream,
    minor_opcode: u8,
    sequence: u16,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // XkbUseExtension (query extension)
            if data.len() >= 4 {
                let wanted_major = u16::from_le_bytes([data[0], data[1]]);
                let wanted_minor = u16::from_le_bytes([data[2], data[3]]);
                log::debug!(
                    "XKB: UseExtension wanted_major={} wanted_minor={}",
                    wanted_major,
                    wanted_minor
                );
            }
            // Return XKB version 1.0 as supported
            let reply = encode_xkb_use_extension_reply(sequence, true, 1, 0);
            stream.write_all(&reply)?;
        }
        _ => {
            log::debug!("XKB: Unhandled minor opcode {}", minor_opcode);
            // For most XKB requests, we don't need to send a reply
            // The important one is UseExtension (opcode 0)
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

fn encode_xkb_use_extension_reply(
    sequence: u16,
    supported: bool,
    server_major: u16,
    server_minor: u16,
) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = if supported { 1 } else { 0 }; // supported
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(server_major));
    buffer[10..12].copy_from_slice(&write_u16_le(server_minor));
    buffer
}

/// Encode a PICTFORMINFO structure (28 bytes)
#[allow(clippy::too_many_arguments)]
fn encode_pictforminfo(
    id: u32,
    format_type: u8,
    depth: u8,
    red_shift: u16,
    red_mask: u16,
    green_shift: u16,
    green_mask: u16,
    blue_shift: u16,
    blue_mask: u16,
    alpha_shift: u16,
    alpha_mask: u16,
    colormap: u32,
) -> Vec<u8> {
    let mut buf = vec![0u8; 28];
    buf[0..4].copy_from_slice(&write_u32_le(id));
    buf[4] = format_type; // 0=indexed, 1=direct
    buf[5] = depth;
    // buf[6..8] unused
    // DIRECTFORMAT starts at offset 8 (16 bytes)
    buf[8..10].copy_from_slice(&write_u16_le(red_shift));
    buf[10..12].copy_from_slice(&write_u16_le(red_mask));
    buf[12..14].copy_from_slice(&write_u16_le(green_shift));
    buf[14..16].copy_from_slice(&write_u16_le(green_mask));
    buf[16..18].copy_from_slice(&write_u16_le(blue_shift));
    buf[18..20].copy_from_slice(&write_u16_le(blue_mask));
    buf[20..22].copy_from_slice(&write_u16_le(alpha_shift));
    buf[22..24].copy_from_slice(&write_u16_le(alpha_mask));
    buf[24..28].copy_from_slice(&write_u32_le(colormap));
    buf
}

fn encode_render_query_pict_formats_reply(sequence: u16) -> Vec<u8> {
    // Define picture formats we support
    // Format IDs start at 1 (0 is reserved for None)
    let formats: Vec<Vec<u8>> = vec![
        // Format 1: 32-bit ARGB (depth 32, with alpha)
        encode_pictforminfo(
            1,    // id
            1,    // type: direct
            32,   // depth
            16,   // red_shift
            0xff, // red_mask
            8,    // green_shift
            0xff, // green_mask
            0,    // blue_shift
            0xff, // blue_mask
            24,   // alpha_shift
            0xff, // alpha_mask
            0,    // colormap
        ),
        // Format 2: 24-bit RGB (depth 24, no alpha)
        encode_pictforminfo(
            2,    // id
            1,    // type: direct
            24,   // depth
            16,   // red_shift
            0xff, // red_mask
            8,    // green_shift
            0xff, // green_mask
            0,    // blue_shift
            0xff, // blue_mask
            0,    // alpha_shift
            0,    // alpha_mask
            0,    // colormap
        ),
        // Format 3: 8-bit alpha only
        encode_pictforminfo(
            3,    // id
            1,    // type: direct
            8,    // depth
            0,    // red_shift
            0,    // red_mask
            0,    // green_shift
            0,    // green_mask
            0,    // blue_shift
            0,    // blue_mask
            0,    // alpha_shift
            0xff, // alpha_mask
            0,    // colormap
        ),
        // Format 4: 1-bit alpha
        encode_pictforminfo(
            4,   // id
            1,   // type: direct
            1,   // depth
            0,   // red_shift
            0,   // red_mask
            0,   // green_shift
            0,   // green_mask
            0,   // blue_shift
            0,   // blue_mask
            0,   // alpha_shift
            0x1, // alpha_mask
            0,   // colormap
        ),
    ];

    let num_formats = formats.len() as u32;

    // Build screen info with depths and visuals
    // PICTSCREEN: num_depths(4) + fallback(4) + depths
    // PICTDEPTH: depth(1) + unused(1) + num_visuals(2) + unused(4) + visuals
    // PICTVISUAL: visual(4) + format(4)

    // We have one screen with depths 24 and 32, each with one visual
    // Visual IDs: use 0x21 for TrueColor (typical default)
    let visual_id: u32 = 0x21;

    // Build depth 24 with one visual pointing to format 2 (24-bit RGB)
    let mut depth24 = vec![0u8; 8]; // PICTDEPTH header
    depth24[0] = 24; // depth
    depth24[2..4].copy_from_slice(&write_u16_le(1)); // num_visuals = 1
                                                     // PICTVISUAL for depth 24
    let mut visual24 = vec![0u8; 8];
    visual24[0..4].copy_from_slice(&write_u32_le(visual_id));
    visual24[4..8].copy_from_slice(&write_u32_le(2)); // format 2 (24-bit RGB)
    depth24.extend(visual24);

    // Build depth 32 with one visual pointing to format 1 (32-bit ARGB)
    let mut depth32 = vec![0u8; 8]; // PICTDEPTH header
    depth32[0] = 32; // depth
    depth32[2..4].copy_from_slice(&write_u16_le(1)); // num_visuals = 1
                                                     // PICTVISUAL for depth 32
    let mut visual32 = vec![0u8; 8];
    visual32[0..4].copy_from_slice(&write_u32_le(visual_id + 1)); // different visual
    visual32[4..8].copy_from_slice(&write_u32_le(1)); // format 1 (32-bit ARGB)
    depth32.extend(visual32);

    // Build screen info
    let mut screen = vec![0u8; 8]; // PICTSCREEN header
    screen[0..4].copy_from_slice(&write_u32_le(2)); // num_depths = 2
    screen[4..8].copy_from_slice(&write_u32_le(2)); // fallback format = 2 (24-bit RGB)
    screen.extend(depth24);
    screen.extend(depth32);

    let num_screens: u32 = 1;
    let num_depths: u32 = 2;
    let num_visuals: u32 = 2;
    let num_subpixels: u32 = 0; // No subpixel info

    // Calculate total size of variable-length data
    let formats_size: usize = formats.iter().map(|f| f.len()).sum();
    let screen_size = screen.len();
    let subpixels_size: usize = 0;
    let total_extra = formats_size + screen_size + subpixels_size;

    // Reply length is in 4-byte units, after the first 32 bytes
    let reply_length = total_extra / 4;

    // Build the reply
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(num_formats));
    buffer[12..16].copy_from_slice(&write_u32_le(num_screens));
    buffer[16..20].copy_from_slice(&write_u32_le(num_depths));
    buffer[20..24].copy_from_slice(&write_u32_le(num_visuals));
    buffer[24..28].copy_from_slice(&write_u32_le(num_subpixels));
    // buffer[28..32] unused

    // Append formats
    for format in formats {
        buffer.extend(format);
    }

    // Append screen info
    buffer.extend(screen);

    // No subpixels

    buffer
}

fn encode_render_query_filters_reply(sequence: u16) -> Vec<u8> {
    // Provide common filter names that X11 apps expect
    // Filter names: "nearest", "bilinear", "convolution", "fast", "good", "best"
    let filters = ["nearest", "bilinear", "convolution", "fast", "good", "best"];

    // Calculate alias count (we provide standard aliases)
    let aliases: [(u16, &str); 3] = [
        (0, "fast"), // fast -> nearest
        (1, "good"), // good -> bilinear
        (1, "best"), // best -> bilinear
    ];

    // Build the filter name list with padding
    let mut filter_data = Vec::new();
    for name in &filters {
        filter_data.push(name.len() as u8);
        filter_data.extend(name.as_bytes());
    }
    // Pad to 4-byte boundary
    while filter_data.len() % 4 != 0 {
        filter_data.push(0);
    }

    // Build alias list (each is 2 bytes)
    let mut alias_data = Vec::new();
    for (idx, _) in &aliases {
        alias_data.extend(write_u16_le(*idx));
    }
    // Pad to 4-byte boundary
    while alias_data.len() % 4 != 0 {
        alias_data.push(0);
    }

    let num_filters = filters.len() as u32;
    let num_aliases = aliases.len() as u32;
    let total_extra = filter_data.len() + alias_data.len();
    let reply_length = total_extra / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(num_aliases));
    buffer[12..16].copy_from_slice(&write_u32_le(num_filters));
    // buffer[16..32] unused

    buffer.extend(alias_data);
    buffer.extend(filter_data);

    buffer
}
