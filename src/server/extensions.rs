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
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // SyncInitialize
            log::debug!("SYNC: Initialize");
            let reply = encode_sync_initialize_reply(sequence);
            stream.write_all(&reply)?;
        }
        1 => {
            // ListSystemCounters
            log::debug!("SYNC: ListSystemCounters");
            let reply = encode_sync_list_system_counters_reply(sequence);
            stream.write_all(&reply)?;
        }
        2 => {
            // CreateCounter
            if data.len() >= 12 {
                let counter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let initial_hi = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let initial_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "SYNC: CreateCounter counter=0x{:x} initial={}:{}",
                    counter,
                    initial_hi,
                    initial_lo
                );
            }
            // No reply
        }
        3 => {
            // SetCounter
            if data.len() >= 12 {
                let counter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let value_hi = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let value_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "SYNC: SetCounter counter=0x{:x} value={}:{}",
                    counter,
                    value_hi,
                    value_lo
                );
            }
            // No reply
        }
        4 => {
            // ChangeCounter
            if data.len() >= 12 {
                let counter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let amount_hi = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let amount_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "SYNC: ChangeCounter counter=0x{:x} amount={}:{}",
                    counter,
                    amount_hi,
                    amount_lo
                );
            }
            // No reply
        }
        5 => {
            // QueryCounter
            if data.len() >= 4 {
                let counter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: QueryCounter counter=0x{:x}", counter);
                let reply = encode_sync_query_counter_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        6 => {
            // DestroyCounter
            if data.len() >= 4 {
                let counter = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: DestroyCounter counter=0x{:x}", counter);
            }
            // No reply
        }
        7 => {
            // Await (wait on triggers)
            log::debug!("SYNC: Await (parsed, no wait)");
            // No reply - would block until conditions met, we just return immediately
        }
        8 => {
            // CreateAlarm
            if data.len() >= 4 {
                let alarm = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: CreateAlarm alarm=0x{:x}", alarm);
            }
            // No reply
        }
        9 => {
            // ChangeAlarm
            if data.len() >= 4 {
                let alarm = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: ChangeAlarm alarm=0x{:x}", alarm);
            }
            // No reply
        }
        10 => {
            // QueryAlarm
            if data.len() >= 4 {
                let alarm = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: QueryAlarm alarm=0x{:x}", alarm);
                let reply = encode_sync_query_alarm_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        11 => {
            // DestroyAlarm
            if data.len() >= 4 {
                let alarm = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: DestroyAlarm alarm=0x{:x}", alarm);
            }
            // No reply
        }
        12 => {
            // SetPriority
            if data.len() >= 8 {
                let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let priority = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!("SYNC: SetPriority id=0x{:x} priority={}", id, priority);
            }
            // No reply
        }
        13 => {
            // GetPriority
            if data.len() >= 4 {
                let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: GetPriority id=0x{:x}", id);
                let reply = encode_sync_get_priority_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        14 => {
            // CreateFence
            if data.len() >= 9 {
                let drawable = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let fence = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let initially_triggered = data[8] != 0;
                log::debug!(
                    "SYNC: CreateFence drawable=0x{:x} fence=0x{:x} triggered={}",
                    drawable,
                    fence,
                    initially_triggered
                );
            }
            // No reply
        }
        15 => {
            // TriggerFence
            if data.len() >= 4 {
                let fence = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: TriggerFence fence=0x{:x}", fence);
            }
            // No reply
        }
        16 => {
            // ResetFence
            if data.len() >= 4 {
                let fence = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: ResetFence fence=0x{:x}", fence);
            }
            // No reply
        }
        17 => {
            // DestroyFence
            if data.len() >= 4 {
                let fence = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: DestroyFence fence=0x{:x}", fence);
            }
            // No reply
        }
        18 => {
            // QueryFence
            if data.len() >= 4 {
                let fence = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("SYNC: QueryFence fence=0x{:x}", fence);
                let reply = encode_sync_query_fence_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        19 => {
            // AwaitFence
            log::debug!("SYNC: AwaitFence (parsed, no wait)");
            // No reply - would block until fences triggered, we just return immediately
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
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // XFixesQueryVersion
            log::debug!("XFIXES: QueryVersion");
            let reply = encode_xfixes_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        1 => {
            // XFixesChangeSaveSet
            if data.len() >= 8 {
                let mode = data[0];
                let target = data[1];
                let map = data[2];
                let window = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "XFIXES: ChangeSaveSet mode={} target={} map={} window=0x{:x}",
                    mode,
                    target,
                    map,
                    window
                );
            }
            // No reply
        }
        2 => {
            // XFixesSelectSelectionInput
            if data.len() >= 12 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let selection = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let event_mask = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: SelectSelectionInput window=0x{:x} selection={} event_mask=0x{:x}",
                    window,
                    selection,
                    event_mask
                );
            }
            // No reply - enables SelectionNotify events
        }
        3 => {
            // XFixesSelectCursorInput
            if data.len() >= 8 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let event_mask = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "XFIXES: SelectCursorInput window=0x{:x} event_mask=0x{:x}",
                    window,
                    event_mask
                );
            }
            // No reply - enables CursorNotify events
        }
        4 => {
            // XFixesGetCursorImage
            log::debug!("XFIXES: GetCursorImage");
            // Return a minimal cursor image (1x1 transparent)
            let reply = encode_xfixes_get_cursor_image_reply(sequence);
            stream.write_all(&reply)?;
        }
        5 => {
            // XFixesCreateRegion
            if data.len() >= 4 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                // Rectangles follow at data[4..]
                let num_rects = (data.len() - 4) / 8;
                log::debug!(
                    "XFIXES: CreateRegion region=0x{:x} num_rects={}",
                    region,
                    num_rects
                );
            }
            // No reply
        }
        6 => {
            // XFixesCreateRegionFromBitmap
            if data.len() >= 8 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let bitmap = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "XFIXES: CreateRegionFromBitmap region=0x{:x} bitmap=0x{:x}",
                    region,
                    bitmap
                );
            }
            // No reply
        }
        7 => {
            // XFixesCreateRegionFromWindow
            if data.len() >= 9 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let window = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let kind = data[8];
                log::debug!(
                    "XFIXES: CreateRegionFromWindow region=0x{:x} window=0x{:x} kind={}",
                    region,
                    window,
                    kind
                );
            }
            // No reply
        }
        8 => {
            // XFixesCreateRegionFromGC
            if data.len() >= 8 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let gc = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "XFIXES: CreateRegionFromGC region=0x{:x} gc=0x{:x}",
                    region,
                    gc
                );
            }
            // No reply
        }
        9 => {
            // XFixesCreateRegionFromPicture
            if data.len() >= 8 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let picture = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "XFIXES: CreateRegionFromPicture region=0x{:x} picture=0x{:x}",
                    region,
                    picture
                );
            }
            // No reply
        }
        10 => {
            // XFixesDestroyRegion
            if data.len() >= 4 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: DestroyRegion region=0x{:x}", region);
            }
            // No reply
        }
        11 => {
            // XFixesSetRegion
            if data.len() >= 4 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let num_rects = (data.len() - 4) / 8;
                log::debug!(
                    "XFIXES: SetRegion region=0x{:x} num_rects={}",
                    region,
                    num_rects
                );
            }
            // No reply
        }
        12 => {
            // XFixesCopyRegion
            if data.len() >= 8 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let dst = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!("XFIXES: CopyRegion src=0x{:x} dst=0x{:x}", src, dst);
            }
            // No reply
        }
        13 => {
            // XFixesUnionRegion
            if data.len() >= 12 {
                let src1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let src2 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let dst = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: UnionRegion src1=0x{:x} src2=0x{:x} dst=0x{:x}",
                    src1,
                    src2,
                    dst
                );
            }
            // No reply
        }
        14 => {
            // XFixesIntersectRegion
            if data.len() >= 12 {
                let src1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let src2 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let dst = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: IntersectRegion src1=0x{:x} src2=0x{:x} dst=0x{:x}",
                    src1,
                    src2,
                    dst
                );
            }
            // No reply
        }
        15 => {
            // XFixesSubtractRegion
            if data.len() >= 12 {
                let src1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let src2 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let dst = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: SubtractRegion src1=0x{:x} src2=0x{:x} dst=0x{:x}",
                    src1,
                    src2,
                    dst
                );
            }
            // No reply
        }
        16 => {
            // XFixesInvertRegion
            if data.len() >= 20 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                // bounds: x, y, width, height at 4..12
                let dst = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
                log::debug!("XFIXES: InvertRegion src=0x{:x} dst=0x{:x}", src, dst);
            }
            // No reply
        }
        17 => {
            // XFixesTranslateRegion
            if data.len() >= 8 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let dx = i16::from_le_bytes([data[4], data[5]]);
                let dy = i16::from_le_bytes([data[6], data[7]]);
                log::debug!(
                    "XFIXES: TranslateRegion region=0x{:x} dx={} dy={}",
                    region,
                    dx,
                    dy
                );
            }
            // No reply
        }
        18 => {
            // XFixesRegionExtents
            if data.len() >= 8 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let dst = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!("XFIXES: RegionExtents src=0x{:x} dst=0x{:x}", src, dst);
            }
            // No reply
        }
        19 => {
            // XFixesFetchRegion
            if data.len() >= 4 {
                let region = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: FetchRegion region=0x{:x}", region);
                // Return empty region (bounding box 0,0,0,0, no rectangles)
                let reply = encode_xfixes_fetch_region_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        20 => {
            // XFixesSetGCClipRegion
            if data.len() >= 12 {
                let gc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let x_origin = i16::from_le_bytes([data[4], data[5]]);
                let y_origin = i16::from_le_bytes([data[6], data[7]]);
                let region = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: SetGCClipRegion gc=0x{:x} origin=({},{}) region=0x{:x}",
                    gc,
                    x_origin,
                    y_origin,
                    region
                );
            }
            // No reply
        }
        21 => {
            // XFixesSetWindowShapeRegion
            if data.len() >= 16 {
                let dst = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let kind = data[4];
                let x_offset = i16::from_le_bytes([data[8], data[9]]);
                let y_offset = i16::from_le_bytes([data[10], data[11]]);
                let region = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                log::debug!(
                    "XFIXES: SetWindowShapeRegion window=0x{:x} kind={} offset=({},{}) region=0x{:x}",
                    dst,
                    kind,
                    x_offset,
                    y_offset,
                    region
                );
            }
            // No reply
        }
        22 => {
            // XFixesSetPictureClipRegion
            if data.len() >= 12 {
                let picture = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let x_origin = i16::from_le_bytes([data[4], data[5]]);
                let y_origin = i16::from_le_bytes([data[6], data[7]]);
                let region = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "XFIXES: SetPictureClipRegion picture=0x{:x} origin=({},{}) region=0x{:x}",
                    picture,
                    x_origin,
                    y_origin,
                    region
                );
            }
            // No reply
        }
        23 => {
            // XFixesSetCursorName
            if data.len() >= 6 {
                let cursor = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let name_len = u16::from_le_bytes([data[4], data[5]]) as usize;
                let name = if data.len() >= 8 + name_len {
                    String::from_utf8_lossy(&data[8..8 + name_len]).to_string()
                } else {
                    String::new()
                };
                log::debug!("XFIXES: SetCursorName cursor=0x{:x} name={}", cursor, name);
            }
            // No reply
        }
        24 => {
            // XFixesGetCursorName
            if data.len() >= 4 {
                let cursor = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: GetCursorName cursor=0x{:x}", cursor);
                // Return empty name
                let reply = encode_xfixes_get_cursor_name_reply(sequence, 0, "");
                stream.write_all(&reply)?;
            }
        }
        25 => {
            // XFixesGetCursorImageAndName
            log::debug!("XFIXES: GetCursorImageAndName");
            // Return minimal cursor info with empty name
            let reply = encode_xfixes_get_cursor_image_and_name_reply(sequence);
            stream.write_all(&reply)?;
        }
        26 => {
            // XFixesChangeCursor
            if data.len() >= 8 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let dst = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!("XFIXES: ChangeCursor src=0x{:x} dst=0x{:x}", src, dst);
            }
            // No reply
        }
        27 => {
            // XFixesChangeCursorByName
            if data.len() >= 6 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let name_len = u16::from_le_bytes([data[4], data[5]]) as usize;
                let name = if data.len() >= 8 + name_len {
                    String::from_utf8_lossy(&data[8..8 + name_len]).to_string()
                } else {
                    String::new()
                };
                log::debug!("XFIXES: ChangeCursorByName src=0x{:x} name={}", src, name);
            }
            // No reply
        }
        28 => {
            // XFixesExpandRegion
            if data.len() >= 16 {
                let src = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let dst = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let left = u16::from_le_bytes([data[8], data[9]]);
                let right = u16::from_le_bytes([data[10], data[11]]);
                let top = u16::from_le_bytes([data[12], data[13]]);
                let bottom = u16::from_le_bytes([data[14], data[15]]);
                log::debug!(
                    "XFIXES: ExpandRegion src=0x{:x} dst=0x{:x} l={} r={} t={} b={}",
                    src,
                    dst,
                    left,
                    right,
                    top,
                    bottom
                );
            }
            // No reply
        }
        29 => {
            // XFixesHideCursor
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: HideCursor window=0x{:x}", window);
                // TODO: Actually hide cursor via backend when supported
            }
            // No reply
        }
        30 => {
            // XFixesShowCursor
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: ShowCursor window=0x{:x}", window);
                // TODO: Actually show cursor via backend when supported
            }
            // No reply
        }
        31 => {
            // XFixesCreatePointerBarrier (version 5.0+)
            if data.len() >= 28 {
                let barrier = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let window = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let x1 = i16::from_le_bytes([data[8], data[9]]);
                let y1 = i16::from_le_bytes([data[10], data[11]]);
                let x2 = i16::from_le_bytes([data[12], data[13]]);
                let y2 = i16::from_le_bytes([data[14], data[15]]);
                let directions = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
                log::debug!(
                    "XFIXES: CreatePointerBarrier barrier=0x{:x} window=0x{:x} ({},{}) to ({},{}) directions=0x{:x}",
                    barrier,
                    window,
                    x1, y1, x2, y2,
                    directions
                );
            }
            // No reply
        }
        32 => {
            // XFixesDeletePointerBarrier (version 5.0+)
            if data.len() >= 4 {
                let barrier = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("XFIXES: DeletePointerBarrier barrier=0x{:x}", barrier);
            }
            // No reply
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
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match minor_opcode {
        0 => {
            // RRQueryVersion
            log::debug!("RANDR: QueryVersion");
            let reply = encode_randr_query_version_reply(sequence);
            stream.write_all(&reply)?;
        }
        2 => {
            // RRSelectInput
            if data.len() >= 6 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let enable = u16::from_le_bytes([data[4], data[5]]);
                log::debug!(
                    "RANDR: SelectInput window=0x{:x} enable=0x{:x}",
                    window,
                    enable
                );
            }
            // No reply
        }
        4 => {
            // RRGetScreenSizeRange
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetScreenSizeRange window=0x{:x}", window);
                let reply = encode_randr_get_screen_size_range_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        5 => {
            // RRSetScreenSize
            if data.len() >= 12 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let width = u16::from_le_bytes([data[4], data[5]]);
                let height = u16::from_le_bytes([data[6], data[7]]);
                let mm_width = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                log::debug!(
                    "RANDR: SetScreenSize window=0x{:x} {}x{} ({}mm)",
                    window,
                    width,
                    height,
                    mm_width
                );
            }
            // No reply
        }
        6 => {
            // RRGetScreenResources
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetScreenResources window=0x{:x}", window);
                let reply = encode_randr_get_screen_resources_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        7 => {
            // RRGetOutputInfo
            if data.len() >= 8 {
                let output = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let config_timestamp = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "RANDR: GetOutputInfo output=0x{:x} config_timestamp={}",
                    output,
                    config_timestamp
                );
                let reply = encode_randr_get_output_info_reply(sequence, output);
                stream.write_all(&reply)?;
            }
        }
        8 => {
            // RRListOutputProperties
            if data.len() >= 4 {
                let output = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: ListOutputProperties output=0x{:x}", output);
                let reply = encode_randr_list_output_properties_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        9 => {
            // RRQueryOutputProperty
            if data.len() >= 8 {
                let output = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let property = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "RANDR: QueryOutputProperty output=0x{:x} property={}",
                    output,
                    property
                );
                let reply = encode_randr_query_output_property_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        13 => {
            // RRGetOutputProperty
            if data.len() >= 24 {
                let output = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let property = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "RANDR: GetOutputProperty output=0x{:x} property={}",
                    output,
                    property
                );
                // Return empty property (type=None)
                let reply = encode_randr_get_output_property_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        18 => {
            // RRGetCrtcInfo
            if data.len() >= 8 {
                let crtc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let config_timestamp = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "RANDR: GetCrtcInfo crtc=0x{:x} config_timestamp={}",
                    crtc,
                    config_timestamp
                );
                let reply = encode_randr_get_crtc_info_reply(sequence, crtc);
                stream.write_all(&reply)?;
            }
        }
        20 => {
            // RRGetCrtcGammaSize
            if data.len() >= 4 {
                let crtc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetCrtcGammaSize crtc=0x{:x}", crtc);
                let reply = encode_randr_get_crtc_gamma_size_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        21 => {
            // RRGetCrtcGamma
            if data.len() >= 4 {
                let crtc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetCrtcGamma crtc=0x{:x}", crtc);
                let reply = encode_randr_get_crtc_gamma_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        22 => {
            // RRSetCrtcGamma
            if data.len() >= 6 {
                let crtc = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let size = u16::from_le_bytes([data[4], data[5]]);
                log::debug!("RANDR: SetCrtcGamma crtc=0x{:x} size={}", crtc, size);
            }
            // No reply
        }
        23 => {
            // RRGetScreenResourcesCurrent (same as GetScreenResources but faster)
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetScreenResourcesCurrent window=0x{:x}", window);
                let reply = encode_randr_get_screen_resources_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        29 => {
            // RRGetOutputPrimary
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetOutputPrimary window=0x{:x}", window);
                let reply = encode_randr_get_output_primary_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        30 => {
            // RRGetProviders
            if data.len() >= 4 {
                let window = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                log::debug!("RANDR: GetProviders window=0x{:x}", window);
                let reply = encode_randr_get_providers_reply(sequence);
                stream.write_all(&reply)?;
            }
        }
        31 => {
            // RRGetProviderInfo
            if data.len() >= 8 {
                let provider = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let config_timestamp = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                log::debug!(
                    "RANDR: GetProviderInfo provider=0x{:x} config_timestamp={}",
                    provider,
                    config_timestamp
                );
                let reply = encode_randr_get_provider_info_reply(sequence);
                stream.write_all(&reply)?;
            }
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

/// Encode SYNC ListSystemCounters reply
/// Returns a single system counter "SERVERTIME" for basic compatibility
fn encode_sync_list_system_counters_reply(sequence: u16) -> Vec<u8> {
    // System counter info: COUNTER (4) + resolution_hi (4) + resolution_lo (4) + name_len (2) + pad + name
    let counter_name = b"SERVERTIME";
    let name_len = counter_name.len() as u16;
    let name_padded = (name_len as usize).div_ceil(4) * 4;

    // One system counter: 4 + 4 + 4 + 2 + 2 + name_padded = 16 + name_padded
    let counter_size = 16 + name_padded;
    let reply_length = counter_size / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(1)); // num_counters

    // System counter entry
    buffer.extend(write_u32_le(0x100)); // COUNTER ID
    buffer.extend(write_u32_le(0)); // resolution_hi
    buffer.extend(write_u32_le(1000)); // resolution_lo (1ms)
    buffer.extend(write_u16_le(name_len));
    buffer.extend([0u8; 2]); // pad
    buffer.extend_from_slice(counter_name);
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }

    buffer
}

/// Encode SYNC QueryCounter reply
/// Returns a zero counter value
fn encode_sync_query_counter_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // counter_value_hi
    buffer[12..16].copy_from_slice(&write_u32_le(0)); // counter_value_lo
    buffer
}

/// Encode SYNC QueryAlarm reply
/// Returns default alarm state
fn encode_sync_query_alarm_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32 + 8]; // Base + extra
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(2)); // length (8 extra bytes)
    // trigger counter, value_type, value, test_type
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // counter (None)
    buffer[12..16].copy_from_slice(&write_u32_le(0)); // value_type (Absolute)
    buffer[16..20].copy_from_slice(&write_u32_le(0)); // value_hi
    buffer[20..24].copy_from_slice(&write_u32_le(0)); // value_lo
    buffer[24..28].copy_from_slice(&write_u32_le(0)); // test_type (PositiveTransition)
    buffer[28..32].copy_from_slice(&write_u32_le(0)); // delta_hi
    // delta_lo, events, state in extra data
    buffer[32..36].copy_from_slice(&write_u32_le(0)); // delta_lo
    buffer[36] = 1; // events (true)
    buffer[37] = 1; // state (Active)
    buffer[38..40].copy_from_slice(&write_u16_le(0)); // pad
    buffer
}

/// Encode SYNC GetPriority reply
fn encode_sync_get_priority_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // priority (0 = normal)
    buffer
}

/// Encode SYNC QueryFence reply
fn encode_sync_query_fence_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8] = 1; // triggered (true - fence is always triggered)
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

fn encode_randr_get_screen_size_range_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(320)); // min_width
    buffer[10..12].copy_from_slice(&write_u16_le(200)); // min_height
    buffer[12..14].copy_from_slice(&write_u16_le(8192)); // max_width
    buffer[14..16].copy_from_slice(&write_u16_le(8192)); // max_height
    buffer
}

/// Encode RANDR GetScreenResources reply
/// Returns a single CRTC, single output, and a common mode (1920x1080@60)
fn encode_randr_get_screen_resources_reply(sequence: u16) -> Vec<u8> {
    let timestamp: u32 = 1;
    let config_timestamp: u32 = 1;

    // We'll report 1 CRTC, 1 output, 1 mode
    let crtc_id: u32 = 0x50; // CRTC ID
    let output_id: u32 = 0x60; // Output ID

    // Mode info: 1920x1080@60Hz
    let mode_id: u32 = 0x70;
    let mode_width: u16 = 1920;
    let mode_height: u16 = 1080;
    let mode_dot_clock: u32 = 148500000; // 148.5 MHz
    let mode_hsync_start: u16 = 2008;
    let mode_hsync_end: u16 = 2052;
    let mode_htotal: u16 = 2200;
    let mode_hskew: u16 = 0;
    let mode_vsync_start: u16 = 1084;
    let mode_vsync_end: u16 = 1089;
    let mode_vtotal: u16 = 1125;
    let mode_name_len: u16 = 12; // "1920x1080_60"
    let mode_flags: u32 = 0; // No special flags

    // Mode name: "1920x1080_60" (12 bytes, pad to 12)
    let mode_name = b"1920x1080_60";

    // Calculate sizes
    let num_crtcs: u16 = 1;
    let num_outputs: u16 = 1;
    let num_modes: u16 = 1;
    let names_len: u16 = 12; // mode name length

    // ModeInfo is 32 bytes
    // Extra data: CRTCs (4 bytes each) + Outputs (4 bytes each) + ModeInfos (32 bytes each) + names
    let crtcs_bytes = 4 * num_crtcs as usize;
    let outputs_bytes = 4 * num_outputs as usize;
    let modes_bytes = 32 * num_modes as usize;
    let names_bytes = (names_len as usize).div_ceil(4) * 4; // pad to 4
    let extra_len = crtcs_bytes + outputs_bytes + modes_bytes + names_bytes;
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(timestamp));
    buffer[12..16].copy_from_slice(&write_u32_le(config_timestamp));
    buffer[16..18].copy_from_slice(&write_u16_le(num_crtcs));
    buffer[18..20].copy_from_slice(&write_u16_le(num_outputs));
    buffer[20..22].copy_from_slice(&write_u16_le(num_modes));
    buffer[22..24].copy_from_slice(&write_u16_le(names_len));
    // buffer[24..32] unused

    // Append CRTCs
    buffer.extend(write_u32_le(crtc_id));

    // Append Outputs
    buffer.extend(write_u32_le(output_id));

    // Append ModeInfo (32 bytes)
    buffer.extend(write_u32_le(mode_id));
    buffer.extend(write_u16_le(mode_width));
    buffer.extend(write_u16_le(mode_height));
    buffer.extend(write_u32_le(mode_dot_clock));
    buffer.extend(write_u16_le(mode_hsync_start));
    buffer.extend(write_u16_le(mode_hsync_end));
    buffer.extend(write_u16_le(mode_htotal));
    buffer.extend(write_u16_le(mode_hskew));
    buffer.extend(write_u16_le(mode_vsync_start));
    buffer.extend(write_u16_le(mode_vsync_end));
    buffer.extend(write_u16_le(mode_vtotal));
    buffer.extend(write_u16_le(mode_name_len));
    buffer.extend(write_u32_le(mode_flags));

    // Append mode names (padded)
    buffer.extend_from_slice(mode_name);

    buffer
}

/// Encode RANDR GetOutputInfo reply
fn encode_randr_get_output_info_reply(sequence: u16, _output: u32) -> Vec<u8> {
    let timestamp: u32 = 1;
    let crtc_id: u32 = 0x50; // Current CRTC
    let mm_width: u32 = 527; // ~21 inch at 1920px
    let mm_height: u32 = 296;
    let connection: u8 = 0; // Connected
    let subpixel_order: u8 = 0; // Unknown
    let num_crtcs: u16 = 1;
    let num_modes: u16 = 1;
    let num_preferred: u16 = 1;
    let num_clones: u16 = 0;
    let name = b"default";
    let name_len: u16 = name.len() as u16;

    // Extra data: CRTCs + Modes + Clones + Name
    let crtcs_bytes = 4 * num_crtcs as usize;
    let modes_bytes = 4 * num_modes as usize;
    let clones_bytes = 4 * num_clones as usize;
    let name_bytes = (name_len as usize).div_ceil(4) * 4;
    // Extra data: num_clones (2) + name_len (2) + CRTCs + Modes + Clones + Name
    let extra_len = 4 + crtcs_bytes + modes_bytes + clones_bytes + name_bytes;
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = 0; // status
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(timestamp));
    buffer[12..16].copy_from_slice(&write_u32_le(crtc_id));
    buffer[16..20].copy_from_slice(&write_u32_le(mm_width));
    buffer[20..24].copy_from_slice(&write_u32_le(mm_height));
    buffer[24] = connection;
    buffer[25] = subpixel_order;
    buffer[26..28].copy_from_slice(&write_u16_le(num_crtcs));
    buffer[28..30].copy_from_slice(&write_u16_le(num_modes));
    buffer[30..32].copy_from_slice(&write_u16_le(num_preferred));

    // Extra data: num_clones (2) + name_len (2) + CRTCs + Modes + Clones + Name
    buffer.extend(write_u16_le(num_clones));
    buffer.extend(write_u16_le(name_len));

    // CRTCs
    buffer.extend(write_u32_le(0x50)); // CRTC ID

    // Modes
    buffer.extend(write_u32_le(0x70)); // Mode ID

    // Clones (none)

    // Name (padded)
    buffer.extend_from_slice(name);
    // Pad to 4-byte boundary
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }

    buffer
}

/// Encode RANDR ListOutputProperties reply (empty list)
fn encode_randr_list_output_properties_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(0)); // num_atoms
    buffer
}

/// Encode RANDR QueryOutputProperty reply
fn encode_randr_query_output_property_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8] = 0; // pending
    buffer[9] = 0; // range
    buffer[10] = 0; // immutable
                    // buffer[11..32] unused
    buffer
}

/// Encode RANDR GetOutputProperty reply (empty/not found)
fn encode_randr_get_output_property_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = 0; // format
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0)); // type (None)
    buffer[12..16].copy_from_slice(&write_u32_le(0)); // bytes_after
    buffer[16..20].copy_from_slice(&write_u32_le(0)); // num_items
    buffer
}

/// Encode RANDR GetCrtcInfo reply
fn encode_randr_get_crtc_info_reply(sequence: u16, _crtc: u32) -> Vec<u8> {
    let timestamp: u32 = 1;
    let x: i16 = 0;
    let y: i16 = 0;
    let width: u16 = 1920;
    let height: u16 = 1080;
    let mode_id: u32 = 0x70;
    let rotation: u16 = 1; // RR_Rotate_0
    let rotations: u16 = 1; // Only RR_Rotate_0 supported
    let num_outputs: u16 = 1;
    let num_possible_outputs: u16 = 1;

    let extra_len = 4 * (num_outputs as usize + num_possible_outputs as usize);
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = 0; // status: Success
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(timestamp));
    buffer[12..14].copy_from_slice(&(x as u16).to_le_bytes());
    buffer[14..16].copy_from_slice(&(y as u16).to_le_bytes());
    buffer[16..18].copy_from_slice(&write_u16_le(width));
    buffer[18..20].copy_from_slice(&write_u16_le(height));
    buffer[20..24].copy_from_slice(&write_u32_le(mode_id));
    buffer[24..26].copy_from_slice(&write_u16_le(rotation));
    buffer[26..28].copy_from_slice(&write_u16_le(rotations));
    buffer[28..30].copy_from_slice(&write_u16_le(num_outputs));
    buffer[30..32].copy_from_slice(&write_u16_le(num_possible_outputs));

    // Outputs
    buffer.extend(write_u32_le(0x60)); // Output ID

    // Possible outputs
    buffer.extend(write_u32_le(0x60)); // Output ID

    buffer
}

/// Encode RANDR GetCrtcGammaSize reply
fn encode_randr_get_crtc_gamma_size_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(256)); // size (256 entries)
    buffer
}

/// Encode RANDR GetCrtcGamma reply (linear gamma)
fn encode_randr_get_crtc_gamma_reply(sequence: u16) -> Vec<u8> {
    let size: u16 = 256;
    // Each channel is 256 u16 values = 512 bytes
    // Total extra: 3 * 512 = 1536 bytes = 384 words
    let extra_len = 3 * 256 * 2;
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..10].copy_from_slice(&write_u16_le(size));

    // Generate linear gamma ramp (identity)
    for i in 0..256u16 {
        let val = (i << 8) | i; // Scale 0-255 to 0-65535
        buffer.extend(write_u16_le(val)); // Red
    }
    for i in 0..256u16 {
        let val = (i << 8) | i;
        buffer.extend(write_u16_le(val)); // Green
    }
    for i in 0..256u16 {
        let val = (i << 8) | i;
        buffer.extend(write_u16_le(val)); // Blue
    }

    buffer
}

/// Encode RANDR GetOutputPrimary reply
fn encode_randr_get_output_primary_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(0x60)); // primary output
    buffer
}

/// Encode RANDR GetProviders reply
fn encode_randr_get_providers_reply(sequence: u16) -> Vec<u8> {
    let timestamp: u32 = 1;
    let num_providers: u16 = 1;
    let provider_id: u32 = 0x80;

    let extra_len = 4 * num_providers as usize;
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(timestamp));
    buffer[12..14].copy_from_slice(&write_u16_le(num_providers));

    // Provider IDs
    buffer.extend(write_u32_le(provider_id));

    buffer
}

/// Encode RANDR GetProviderInfo reply
fn encode_randr_get_provider_info_reply(sequence: u16) -> Vec<u8> {
    let timestamp: u32 = 1;
    let capabilities: u32 = 0x0F; // Source Output, Sink Output, Source Offload, Sink Offload
    let num_crtcs: u16 = 1;
    let num_outputs: u16 = 1;
    let num_associated_providers: u16 = 0;
    let name = b"X11Anywhere";
    let name_len: u16 = name.len() as u16;

    let crtcs_bytes = 4 * num_crtcs as usize;
    let outputs_bytes = 4 * num_outputs as usize;
    let providers_bytes = 4 * num_associated_providers as usize;
    let name_bytes = (name_len as usize).div_ceil(4) * 4;
    let extra_len = crtcs_bytes + outputs_bytes + providers_bytes + name_bytes;
    let reply_length = extra_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[1] = 0; // status
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(reply_length as u32));
    buffer[8..12].copy_from_slice(&write_u32_le(timestamp));
    buffer[12..16].copy_from_slice(&write_u32_le(capabilities));
    buffer[16..18].copy_from_slice(&write_u16_le(num_crtcs));
    buffer[18..20].copy_from_slice(&write_u16_le(num_outputs));
    buffer[20..22].copy_from_slice(&write_u16_le(num_associated_providers));
    buffer[22..24].copy_from_slice(&write_u16_le(name_len));

    // CRTCs
    buffer.extend(write_u32_le(0x50));

    // Outputs
    buffer.extend(write_u32_le(0x60));

    // Associated providers (none)

    // Name (padded)
    buffer.extend_from_slice(name);
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }

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

/// Encode GetCursorImage reply
/// Returns a minimal 1x1 transparent cursor
fn encode_xfixes_get_cursor_image_reply(sequence: u16) -> Vec<u8> {
    // Cursor image is 1x1 pixel (transparent)
    let width: u16 = 1;
    let height: u16 = 1;
    let cursor_serial: u32 = 1;

    // Image data is 1 ARGB pixel (4 bytes = 1 word)
    let image_data_len: u32 = 1; // in 4-byte units

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(image_data_len)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(0)); // x hotspot
    buffer[10..12].copy_from_slice(&write_u16_le(0)); // y hotspot
    buffer[12..14].copy_from_slice(&write_u16_le(width));
    buffer[14..16].copy_from_slice(&write_u16_le(height));
    buffer[16..20].copy_from_slice(&write_u32_le(0)); // xhot (fixed-point, unused)
    buffer[20..24].copy_from_slice(&write_u32_le(0)); // yhot (fixed-point, unused)
    buffer[24..28].copy_from_slice(&write_u32_le(cursor_serial));
    // buffer[28..32] unused

    // Append 1 transparent ARGB pixel (4 bytes)
    buffer.extend([0u8; 4]);

    buffer
}

/// Encode FetchRegion reply
/// Returns an empty region (bounding box 0,0,0,0, no rectangles)
fn encode_xfixes_fetch_region_reply(sequence: u16) -> Vec<u8> {
    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(0)); // length (no extra data)
                                                    // Bounding box: x, y, width, height (all 0)
    buffer[8..10].copy_from_slice(&write_u16_le(0)); // x
    buffer[10..12].copy_from_slice(&write_u16_le(0)); // y
    buffer[12..14].copy_from_slice(&write_u16_le(0)); // width
    buffer[14..16].copy_from_slice(&write_u16_le(0)); // height
                                                      // No rectangles follow
    buffer
}

/// Encode GetCursorName reply
fn encode_xfixes_get_cursor_name_reply(sequence: u16, atom: u32, name: &str) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len() as u16;
    // Pad name to 4-byte boundary
    let padded_len = (name_len as usize + 3) & !3;
    let extra_words = padded_len / 4;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(extra_words as u32)); // length
    buffer[8..12].copy_from_slice(&write_u32_le(atom)); // atom
    buffer[12..14].copy_from_slice(&write_u16_le(name_len)); // name length
                                                             // buffer[14..32] unused

    // Append name (padded)
    buffer.extend(name_bytes);
    buffer.resize(32 + padded_len, 0);

    buffer
}

/// Encode GetCursorImageAndName reply
/// Returns a minimal 1x1 transparent cursor with empty name
fn encode_xfixes_get_cursor_image_and_name_reply(sequence: u16) -> Vec<u8> {
    let width: u16 = 1;
    let height: u16 = 1;
    let cursor_serial: u32 = 1;
    let name_len: u16 = 0;

    // Image data is 1 ARGB pixel (4 bytes = 1 word)
    // No name data (name_len = 0)
    let image_data_len: u32 = 1;

    let mut buffer = vec![0u8; 32];
    buffer[0] = 1; // Reply
    buffer[2..4].copy_from_slice(&write_u16_le(sequence));
    buffer[4..8].copy_from_slice(&write_u32_le(image_data_len)); // length
    buffer[8..10].copy_from_slice(&write_u16_le(0)); // x hotspot
    buffer[10..12].copy_from_slice(&write_u16_le(0)); // y hotspot
    buffer[12..14].copy_from_slice(&write_u16_le(width));
    buffer[14..16].copy_from_slice(&write_u16_le(height));
    buffer[16..20].copy_from_slice(&write_u32_le(0)); // xhot
    buffer[20..24].copy_from_slice(&write_u32_le(0)); // yhot
    buffer[24..28].copy_from_slice(&write_u32_le(cursor_serial));
    buffer[28..30].copy_from_slice(&write_u16_le(0)); // cursor-atom (None)
    buffer[30..32].copy_from_slice(&write_u16_le(name_len));

    // Append 1 transparent ARGB pixel (4 bytes)
    buffer.extend([0u8; 4]);

    buffer
}
