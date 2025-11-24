/// Test X11 client to debug protocol implementation
///
/// This client connects to our test server and dumps all protocol data
/// to help debug what's wrong with our SetupReply.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use x11anywhere::protocol::*;

fn dump_bytes(label: &str, data: &[u8]) {
    println!("{}: {} bytes", label, data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("  {:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
        // Padding
        for _ in 0..(16 - chunk.len()) {
            print!("   ");
        }
        print!(" | ");
        for byte in chunk {
            if *byte >= 32 && *byte < 127 {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        println!();
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let display = 99;

    // Connect via abstract socket
    #[cfg(target_os = "linux")]
    let stream = {
        use std::os::unix::ffi::OsStrExt;
        use std::ffi::OsStr;

        let mut abstract_path = vec![0u8];
        abstract_path.extend_from_slice(b"/tmp/.X11-unix/X");
        abstract_path.extend_from_slice(display.to_string().as_bytes());

        let os_str = OsStr::from_bytes(&abstract_path);
        UnixStream::connect(os_str).expect("Failed to connect to server")
    };

    #[cfg(not(target_os = "linux"))]
    let stream = {
        let socket_path = format!("/tmp/.X11-unix/X{}", display);
        UnixStream::connect(&socket_path).expect("Failed to connect to server")
    };

    let mut stream = stream;
    println!("Connected to display :{}", display);

    // Send setup request
    let mut setup_bytes = Vec::new();

    // Byte order (LSB first = 'l')
    setup_bytes.push(b'l');
    setup_bytes.push(0); // Padding

    // Protocol version
    setup_bytes.extend_from_slice(&11u16.to_le_bytes()); // Major
    setup_bytes.extend_from_slice(&0u16.to_le_bytes());  // Minor

    // Authorization (none)
    setup_bytes.extend_from_slice(&0u16.to_le_bytes()); // Auth protocol name length
    setup_bytes.extend_from_slice(&0u16.to_le_bytes()); // Auth protocol data length
    setup_bytes.extend_from_slice(&[0u8; 2]); // Padding

    println!("\n=== Sending SetupRequest ===");
    dump_bytes("Setup request", &setup_bytes);

    stream.write_all(&setup_bytes).expect("Failed to send setup request");
    println!("Setup request sent");

    // Read setup reply header (8 bytes minimum)
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).expect("Failed to read setup reply header");

    println!("\n=== Setup Reply Header ===");
    dump_bytes("Header", &header);

    let status = header[0];
    let detail = header[1];
    let protocol_major = u16::from_le_bytes([header[2], header[3]]);
    let protocol_minor = u16::from_le_bytes([header[4], header[5]]);
    let additional_length = u16::from_le_bytes([header[6], header[7]]);

    println!("Status: {} (0=Failed, 1=Success, 2=Authenticate)", status);
    println!("Detail: {}", detail);
    println!("Protocol version: {}.{}", protocol_major, protocol_minor);
    println!("Additional data length: {} (= {} bytes)", additional_length, additional_length * 4);

    if status == 0 {
        // Failed
        let reason_len = detail as usize;
        let data_len = (additional_length as usize) * 4;
        let mut data = vec![0u8; data_len];
        stream.read_exact(&mut data).expect("Failed to read failure reason");

        println!("\n=== Setup Failed ===");
        dump_bytes("Failure data", &data);

        let reason = String::from_utf8_lossy(&data[..reason_len.min(data.len())]);
        println!("Reason: {}", reason);
    } else if status == 1 {
        // Success
        let data_len = (additional_length as usize) * 4;
        let mut data = vec![0u8; data_len];
        stream.read_exact(&mut data).expect("Failed to read setup success data");

        println!("\n=== Setup Success Data ===");
        dump_bytes("Success data", &data);

        // Parse the success data
        println!("\n=== Parsing Setup Success ===");

        let release_number = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let resource_id_base = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let resource_id_mask = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let motion_buffer_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let vendor_len = u16::from_le_bytes([data[16], data[17]]) as usize;
        let max_request_length = u16::from_le_bytes([data[18], data[19]]);
        let num_screens = data[20];
        let num_formats = data[21];
        let image_byte_order = data[22];
        let bitmap_format_bit_order = data[23];
        let bitmap_format_scanline_unit = data[24];
        let bitmap_format_scanline_pad = data[25];
        let min_keycode = data[26];
        let max_keycode = data[27];

        println!("Release number: {}", release_number);
        println!("Resource ID base: 0x{:08x}", resource_id_base);
        println!("Resource ID mask: 0x{:08x}", resource_id_mask);
        println!("Motion buffer size: {}", motion_buffer_size);
        println!("Vendor length: {}", vendor_len);
        println!("Max request length: {}", max_request_length);
        println!("Number of screens: {}", num_screens);
        println!("Number of formats: {}", num_formats);
        println!("Image byte order: {} (0=LSB, 1=MSB)", image_byte_order);
        println!("Bitmap format bit order: {}", bitmap_format_bit_order);
        println!("Bitmap scanline unit: {}", bitmap_format_scanline_unit);
        println!("Bitmap scanline pad: {}", bitmap_format_scanline_pad);
        println!("Min keycode: {}", min_keycode);
        println!("Max keycode: {}", max_keycode);

        // Vendor string starts at offset 32
        if data.len() > 32 {
            let vendor_padded = padded_len(vendor_len);
            let vendor_end = 32 + vendor_len;
            if vendor_end <= data.len() {
                let vendor = String::from_utf8_lossy(&data[32..vendor_end]);
                println!("Vendor: '{}'", vendor);
            }

            // Formats start after vendor (padded)
            let mut offset = 32 + vendor_padded;
            println!("\n=== Pixmap Formats ===");
            for i in 0..num_formats {
                if offset + 8 <= data.len() {
                    let depth = data[offset];
                    let bpp = data[offset + 1];
                    let scanline_pad = data[offset + 2];
                    println!("Format {}: depth={}, bpp={}, scanline_pad={}",
                             i, depth, bpp, scanline_pad);
                    offset += 8;
                }
            }

            // Screens
            println!("\n=== Screens ===");
            for i in 0..num_screens {
                if offset + 40 <= data.len() {
                    let root = u32::from_le_bytes([
                        data[offset], data[offset+1], data[offset+2], data[offset+3]
                    ]);
                    let default_colormap = u32::from_le_bytes([
                        data[offset+4], data[offset+5], data[offset+6], data[offset+7]
                    ]);
                    let white_pixel = u32::from_le_bytes([
                        data[offset+8], data[offset+9], data[offset+10], data[offset+11]
                    ]);
                    let black_pixel = u32::from_le_bytes([
                        data[offset+12], data[offset+13], data[offset+14], data[offset+15]
                    ]);
                    let width = u16::from_le_bytes([data[offset+20], data[offset+21]]);
                    let height = u16::from_le_bytes([data[offset+22], data[offset+23]]);
                    let num_depths = data[offset+39];

                    println!("Screen {}:", i);
                    println!("  Root window: 0x{:08x}", root);
                    println!("  Default colormap: 0x{:08x}", default_colormap);
                    println!("  White pixel: 0x{:06x}", white_pixel);
                    println!("  Black pixel: 0x{:06x}", black_pixel);
                    println!("  Size: {}x{} pixels", width, height);
                    println!("  Number of depths: {}", num_depths);

                    offset += 40;

                    // Parse depths
                    for j in 0..num_depths {
                        if offset + 8 <= data.len() {
                            let depth = data[offset];
                            let num_visuals = u16::from_le_bytes([data[offset+2], data[offset+3]]);
                            println!("  Depth {}: depth={}, {} visuals", j, depth, num_visuals);
                            offset += 8;

                            // Parse visuals (24 bytes each)
                            for k in 0..num_visuals {
                                if offset + 24 <= data.len() {
                                    let visual_id = u32::from_le_bytes([
                                        data[offset], data[offset+1], data[offset+2], data[offset+3]
                                    ]);
                                    let class = data[offset+4];
                                    let bits_per_rgb = data[offset+5];

                                    println!("    Visual {}: id=0x{:08x}, class={}, bits_per_rgb={}",
                                             k, visual_id, class, bits_per_rgb);
                                    offset += 24;
                                }
                            }
                        }
                    }
                }
            }
        }

        println!("\n=== Setup Complete ===");
        println!("Connection established successfully!");

        // Try to send a simple request (QueryExtension for "BIG-REQUESTS")
        println!("\n=== Sending QueryExtension request ===");
        let ext_name = "BIG-REQUESTS";
        let name_len = ext_name.len() as u16;
        let name_padded = padded_len(ext_name.len());

        let mut request = Vec::new();
        request.push(98); // QueryExtension opcode
        request.push(0);  // unused
        request.extend_from_slice(&((2 + (name_padded / 4)) as u16).to_le_bytes()); // length in 4-byte units
        request.extend_from_slice(&name_len.to_le_bytes());
        request.extend_from_slice(&[0u8; 2]); // padding
        request.extend_from_slice(ext_name.as_bytes());
        for _ in 0..pad(ext_name.len()) {
            request.push(0);
        }

        dump_bytes("QueryExtension request", &request);
        stream.write_all(&request).expect("Failed to send request");

        // Read reply
        let mut reply = [0u8; 32];
        match stream.read_exact(&mut reply) {
            Ok(_) => {
                println!("\n=== Reply received ===");
                dump_bytes("Reply", &reply);
            }
            Err(e) => {
                println!("\n=== No reply received ===");
                println!("Error: {}", e);
            }
        }
    }

    println!("\n=== Test complete ===");
}
