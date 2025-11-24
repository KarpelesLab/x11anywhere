/// X11 Passthrough Server with Debugging
///
/// This server uses the X11 backend to connect to a real X server
/// and returns the EXACT SetupSuccess data from that server.
/// This helps us debug what a real X server sends.

use x11anywhere::backend::{Backend, x11::X11Backend};
use x11anywhere::protocol::*;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::os::fd::{FromRawFd, AsRawFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn dump_bytes(direction: &str, seq: usize, data: &[u8]) {
    log::info!("\n{} #{}: {} bytes", direction, seq, data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
        let ascii: String = chunk.iter().map(|b| {
            if *b >= 32 && *b < 127 {
                *b as char
            } else {
                '.'
            }
        }).collect();
        log::info!("  {:04x}: {:48} | {}", i * 16, hex, ascii);
    }
}

fn handle_client(mut stream: UnixStream, client_id: usize, backend: &mut X11Backend) {
    log::info!("[Client {}] Connected", client_id);

    // Read setup request from client
    let setup_request = match SetupRequest::parse(&mut stream) {
        Ok(req) => {
            log::info!("[Client {}] Setup request: {:?}", client_id, req.byte_order);
            req
        }
        Err(e) => {
            log::error!("[Client {}] Failed to parse setup request: {}", client_id, e);
            return;
        }
    };

    let byte_order = setup_request.byte_order;

    // Get the setup info from the real X server (already parsed by backend.init())
    let setup_info = backend.setup_info().expect("Backend not initialized");

    log::info!("[Client {}] Sending setup from real X server:", client_id);
    log::info!("  Vendor: {}", setup_info.vendor);
    log::info!("  Resource ID base: 0x{:08x}", setup_info.resource_id_base);
    log::info!("  Resource ID mask: 0x{:08x}", setup_info.resource_id_mask);
    log::info!("  Formats: {}", setup_info.pixmap_formats.len());
    log::info!("  Screens: {}", setup_info.roots.len());

    // Encode to buffer first so we can dump it
    let mut encoded_buffer = Vec::new();
    if let Err(e) = SetupResponse::Success(setup_info.clone()).encode(&mut encoded_buffer, byte_order) {
        log::error!("[Client {}] Failed to encode setup reply: {}", client_id, e);
        return;
    }

    log::info!("[Client {}] Encoded {} bytes:", client_id, encoded_buffer.len());
    // Dump first 64 bytes as hex
    for (i, chunk) in encoded_buffer.chunks(16).take(4).enumerate() {
        let hex: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
        log::info!("  {:04x}: {}", i * 16, hex);
    }

    // Send it
    if let Err(e) = stream.write_all(&encoded_buffer) {
        log::error!("[Client {}] Failed to send setup reply: {}", client_id, e);
        return;
    }

    log::info!("[Client {}] Setup reply sent successfully", client_id);

    // Now set up bidirectional forwarding
    log::info!("[Client {}] Setting up bidirectional proxy...", client_id);

    let mut server = match backend.clone_connection() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[Client {}] Failed to clone backend connection: {}", client_id, e);
            return;
        }
    };

    let mut server_reader = match server.try_clone() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[Client {}] Failed to clone server stream: {}", client_id, e);
            return;
        }
    };

    let mut client_writer = match stream.try_clone() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[Client {}] Failed to clone client stream: {}", client_id, e);
            return;
        }
    };

    let seq_client_to_server = Arc::new(AtomicUsize::new(0));
    let seq_server_to_client = Arc::new(AtomicUsize::new(0));
    let seq_s2c = seq_server_to_client.clone();

    // Thread to forward server -> client
    let cid = client_id;
    let server_to_client_thread = thread::spawn(move || {
        let mut buffer = vec![0u8; 8192];
        loop {
            match server_reader.read(&mut buffer) {
                Ok(0) => {
                    log::info!("[Client {}] Server closed connection", cid);
                    break;
                }
                Ok(n) => {
                    let seq = seq_s2c.fetch_add(1, Ordering::SeqCst);
                    dump_bytes(&format!("[Client {}] SERVER -> CLIENT", cid), seq, &buffer[..n]);
                    if let Err(e) = client_writer.write_all(&buffer[..n]) {
                        log::error!("[Client {}] Error writing to client: {}", cid, e);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("[Client {}] Error reading from server: {}", cid, e);
                    break;
                }
            }
        }
    });

    // Main thread forwards client -> server
    let mut buffer = vec![0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                log::info!("[Client {}] Client closed connection", client_id);
                break;
            }
            Ok(n) => {
                let seq = seq_client_to_server.fetch_add(1, Ordering::SeqCst);
                dump_bytes(&format!("[Client {}] CLIENT -> SERVER", client_id), seq, &buffer[..n]);
                if let Err(e) = server.write_all(&buffer[..n]) {
                    log::error!("[Client {}] Error writing to server: {}", client_id, e);
                    break;
                }
            }
            Err(e) => {
                log::error!("[Client {}] Error reading from client: {}", client_id, e);
                break;
            }
        }
    }

    let _ = server_to_client_thread.join();
    log::info!("[Client {}] Disconnected", client_id);
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let proxy_display = 99;
    let target_display = ":0";

    log::info!("X11 Passthrough Server");
    log::info!("Proxy display: :{}", proxy_display);
    log::info!("Target display: {}", target_display);

    // Initialize backend - connect to real X server
    let mut backend = X11Backend::new(target_display).with_debug(true);

    log::info!("Connecting to real X server {}...", target_display);
    if let Err(e) = backend.init() {
        log::error!("Failed to initialize X11 backend: {}", e);
        std::process::exit(1);
    }

    log::info!("Connected to real X server successfully!");

    // Create abstract socket for proxy
    #[cfg(target_os = "linux")]
    {
        use nix::sys::socket::*;

        let socket_fd = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        ).expect("Failed to create socket");

        let abstract_name = format!("/tmp/.X11-unix/X{}", proxy_display);
        let addr = UnixAddr::new_abstract(abstract_name.as_bytes())
            .expect("Failed to create abstract address");

        bind(socket_fd.as_raw_fd(), &addr)
            .expect("Failed to bind abstract socket");

        listen(&socket_fd, Backlog::new(128).unwrap())
            .expect("Failed to listen on socket");

        let listener = unsafe { UnixListener::from_raw_fd(socket_fd.as_raw_fd()) };
        std::mem::forget(socket_fd);

        log::info!("Listening on abstract socket: @/tmp/.X11-unix/X{}", proxy_display);
        log::info!("Test with: DISPLAY=:{} xcalc", proxy_display);
        println!();

        let mut client_counter = 0;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    client_counter += 1;
                    let client_id = client_counter;

                    // Clone setup info for the thread
                    // For now, we'll handle one client at a time
                    // (Multi-client would need Arc<Mutex<Backend>> or similar)
                    handle_client(stream, client_id, &mut backend);
                }
                Err(e) => {
                    log::error!("Connection error: {}", e);
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("This server currently only works on Linux with abstract sockets");
        std::process::exit(1);
    }
}
