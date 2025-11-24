/// X11 Logging Proxy
///
/// This proxy sits between X11 clients and a real X server,
/// logging all protocol data to help us understand the correct format.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn dump_bytes(direction: &str, seq: usize, data: &[u8]) {
    println!("\n{} #{}: {} bytes", direction, seq, data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("  {:04x}: ", i * 16);
        for byte in chunk {
            print!("{:02x} ", byte);
        }
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

fn handle_client(mut client: UnixStream, client_id: usize, real_display_num: usize) {
    println!("\n=== Client {} connected ===", client_id);

    // Connect to real X server via abstract socket
    #[cfg(target_os = "linux")]
    let mut server = {
        use std::os::unix::ffi::OsStrExt;
        use std::ffi::OsStr;

        let mut path_bytes = vec![0u8];
        path_bytes.extend_from_slice(format!("/tmp/.X11-unix/X{}", real_display_num).as_bytes());
        let os_str = OsStr::from_bytes(&path_bytes);

        match UnixStream::connect(os_str) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect to real server :{}: {}", real_display_num, e);
                return;
            }
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut server = {
        let path = format!("/tmp/.X11-unix/X{}", real_display_num);
        match UnixStream::connect(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect to real server: {}", e);
                return;
            }
        }
    };

    println!("Connected to real server: :{}", real_display_num);

    let seq_client_to_server = Arc::new(AtomicUsize::new(0));
    let seq_server_to_client = Arc::new(AtomicUsize::new(0));

    // Clone for the server->client thread
    let mut server_reader = match server.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone server stream: {}", e);
            return;
        }
    };

    let mut client_writer = match client.try_clone() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to clone client stream: {}", e);
            return;
        }
    };

    let seq_s2c = seq_server_to_client.clone();

    // Thread to forward server -> client
    let server_to_client_thread = thread::spawn(move || {
        let mut buffer = vec![0u8; 8192];
        loop {
            match server_reader.read(&mut buffer) {
                Ok(0) => {
                    println!("\n=== Server closed connection ===");
                    break;
                }
                Ok(n) => {
                    let seq = seq_s2c.fetch_add(1, Ordering::SeqCst);
                    dump_bytes("SERVER -> CLIENT", seq, &buffer[..n]);

                    if let Err(e) = client_writer.write_all(&buffer[..n]) {
                        eprintln!("Error writing to client: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread forwards client -> server
    let mut buffer = vec![0u8; 8192];
    loop {
        match client.read(&mut buffer) {
            Ok(0) => {
                println!("\n=== Client closed connection ===");
                break;
            }
            Ok(n) => {
                let seq = seq_client_to_server.fetch_add(1, Ordering::SeqCst);
                dump_bytes("CLIENT -> SERVER", seq, &buffer[..n]);

                if let Err(e) = server.write_all(&buffer[..n]) {
                    eprintln!("Error writing to server: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error reading from client: {}", e);
                break;
            }
        }
    }

    let _ = server_to_client_thread.join();
    println!("\n=== Client {} disconnected ===", client_id);
}

fn main() {
    let proxy_display = std::env::args().nth(1).unwrap_or_else(|| "99".to_string());
    let real_display = std::env::args().nth(2).unwrap_or_else(|| "0".to_string());

    println!("X11 Logging Proxy");
    println!("Proxy display: :{}", proxy_display);
    println!("Real display: :{}", real_display);
    println!();

    // Create abstract socket for proxy
    #[cfg(target_os = "linux")]
    {
        use nix::sys::socket::*;
        use std::os::fd::{FromRawFd, AsRawFd};

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

        println!("Listening on abstract socket: @/tmp/.X11-unix/X{}", proxy_display);
        println!("Forwarding to: /tmp/.X11-unix/X{}", real_display);
        println!();
        println!("Usage: DISPLAY=:{} xcalc", proxy_display);
        println!();

        let real_display_num: usize = real_display.parse().expect("Invalid display number");
        let mut client_counter = 0;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    client_counter += 1;
                    let client_id = client_counter;

                    thread::spawn(move || {
                        handle_client(stream, client_id, real_display_num);
                    });
                }
                Err(e) => {
                    eprintln!("Connection error: {}", e);
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("This proxy currently only works on Linux with abstract sockets");
        std::process::exit(1);
    }
}
