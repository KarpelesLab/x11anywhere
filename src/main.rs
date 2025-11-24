//! X11Anywhere - Main entry point
//!
//! A portable X11 server with modular backend support

use std::env;
use std::process;

// Internal modules
mod backend;
mod connection;
mod protocol;
mod resources;
mod security;
mod server;

use security::SecurityPolicy;

/// Server version
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    println!("X11Anywhere v{}", VERSION);
    println!("A portable X11 server implementation");
    println!();
    println!("Usage: x11anywhere [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -display <n>          Display number (default: 1)");
    println!("  -backend <type>       Backend type (x11, wayland, macos, windows)");
    println!("  -tcp                  Listen on TCP (port 6000 + display)");
    println!("  -unix                 Listen on Unix socket (default on Unix)");
    println!("  -security <level>     Security level: permissive, default, strict");
    println!("  -list-backends        List available backends");
    println!("  -h, --help            Show this help message");
    println!();
    println!("Examples:");
    println!("  x11anywhere -display 1 -backend x11");
    println!("  x11anywhere -display 2 -backend wayland -tcp");
    println!();
}

fn list_backends() {
    println!("Available backends on this platform:");
    let backends = backend::available_backends();
    if backends.is_empty() {
        println!("  (none available)");
        println!();
        println!(
            "Note: Backends are enabled by default but may not be available on this platform."
        );
    } else {
        for backend in backends {
            println!("  - {}", backend);
        }
        println!();
        println!("These backends are enabled by default for your platform.");
    }
    println!();
    println!("To build with specific backends only:");
    println!("  cargo build --no-default-features --features backend-x11");
    println!("  cargo build --no-default-features --features backend-wayland");
    println!();
    println!("Platform defaults:");
    println!("  - Linux/BSD: X11 + Wayland");
    println!("  - macOS: macOS native");
    println!("  - Windows: Windows native");
}

#[derive(Debug)]
struct Config {
    display: u16,
    backend_type: Option<String>,
    listen_tcp: bool,
    listen_unix: bool,
    security: SecurityPolicy,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            display: 1,
            backend_type: None,
            #[cfg(unix)]
            listen_tcp: false,
            #[cfg(not(unix))]
            listen_tcp: true,
            #[cfg(unix)]
            listen_unix: true,
            #[cfg(not(unix))]
            listen_unix: false,
            security: SecurityPolicy::default(),
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let mut config = Config::default();
    let args: Vec<String> = env::args().collect();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "-list-backends" => {
                list_backends();
                process::exit(0);
            }
            "-display" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for -display".to_string());
                }
                config.display = args[i]
                    .parse()
                    .map_err(|_| "Invalid display number".to_string())?;
            }
            "-backend" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for -backend".to_string());
                }
                config.backend_type = Some(args[i].clone());
            }
            "-tcp" => {
                config.listen_tcp = true;
            }
            "-unix" => {
                config.listen_unix = true;
            }
            "-security" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for -security".to_string());
                }
                config.security = match args[i].as_str() {
                    "permissive" => SecurityPolicy::permissive(),
                    "default" => SecurityPolicy::default(),
                    "strict" => SecurityPolicy::strict(),
                    _ => return Err(format!("Invalid security level: {}", args[i])),
                };
            }
            arg => {
                return Err(format!("Unknown option: {}", arg));
            }
        }
        i += 1;
    }

    Ok(config)
}

fn auto_detect_backend() -> Option<String> {
    // Try to auto-detect the best backend for this platform
    let available = backend::available_backends();

    if available.is_empty() {
        return None;
    }

    // Platform-specific preferences
    #[cfg(target_os = "linux")]
    {
        // Prefer Wayland on Linux if available, fall back to X11
        if available.contains(&"wayland") {
            return Some("wayland".to_string());
        }
        if available.contains(&"x11") {
            return Some("x11".to_string());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if available.contains(&"macos") {
            return Some("macos".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        if available.contains(&"windows") {
            return Some("windows".to_string());
        }
    }

    // Fall back to first available
    available.first().map(|s| s.to_string())
}

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command line arguments
    let config = match parse_args() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error: {}", err);
            eprintln!();
            print_usage();
            process::exit(1);
        }
    };

    // Determine backend
    let backend_type = match config.backend_type {
        Some(ref b) => b.clone(),
        None => match auto_detect_backend() {
            Some(b) => {
                log::info!("Auto-detected backend: {}", b);
                b
            }
            None => {
                eprintln!("Error: No backends available!");
                eprintln!("Please compile with backend features enabled.");
                eprintln!();
                list_backends();
                process::exit(1);
            }
        },
    };

    // Validate backend is available
    let available = backend::available_backends();
    if !available.contains(&backend_type.as_str()) {
        eprintln!("Error: Backend '{}' is not available", backend_type);
        eprintln!();
        list_backends();
        process::exit(1);
    }

    log::info!("X11Anywhere v{}", VERSION);
    log::info!("Display: :{}", config.display);
    log::info!("Backend: {}", backend_type);
    log::info!("TCP listening: {}", config.listen_tcp);
    log::info!("Unix socket listening: {}", config.listen_unix);
    log::info!(
        "Security policy: window_isolation={}, global_selections={}",
        config.security.window_isolation,
        config.security.allow_global_selections
    );

    // Initialize backend based on platform
    let backend: Box<dyn backend::Backend> = {
        #[cfg(all(feature = "backend-windows", target_os = "windows"))]
        {
            Box::new(backend::windows::WindowsBackend::new())
        }

        #[cfg(all(feature = "backend-macos", target_os = "macos"))]
        {
            Box::new(backend::macos::MacOSBackend::new())
        }

        #[cfg(all(feature = "backend-x11", target_family = "unix", not(target_os = "macos")))]
        {
            // X11 backend requires a target display
            let target_display = env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
            Box::new(backend::x11::X11Backend::new(&target_display))
        }

        #[cfg(not(any(
            all(feature = "backend-windows", target_os = "windows"),
            all(feature = "backend-macos", target_os = "macos"),
            all(feature = "backend-x11", target_family = "unix", not(target_os = "macos"))
        )))]
        {
            // Fall back to null backend if no real backend is available
            Box::new(backend::null::NullBackend::new())
        }
    };

    // Create server
    let server = match server::Server::new(backend) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: Failed to initialize server: {}", e);
            process::exit(1);
        }
    };

    // Wrap server in Arc<Mutex<>> for thread-safe access
    let server = std::sync::Arc::new(std::sync::Mutex::new(server));

    // Start TCP listener
    if config.listen_tcp {
        let tcp_server = std::sync::Arc::clone(&server);
        log::info!("Starting TCP listener on port {}", 6000 + config.display);

        if let Err(e) = server::listener::start_tcp_listener(config.display, tcp_server) {
            eprintln!("Error: Failed to start TCP listener: {}", e);
            process::exit(1);
        }
    }

    if config.listen_unix {
        log::warn!("Unix socket listener not yet implemented");
    }
}
