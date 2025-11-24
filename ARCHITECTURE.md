# X11Anywhere Architecture

## Overview

X11Anywhere is a portable X11 server implementation in Rust that can run on Linux, macOS, and Windows. It accepts X11 connections and translates them to native display systems through a modular backend architecture.

## Design Goals

1. **Portability**: Run on Linux, macOS, and Windows
2. **Minimal Dependencies**: Keep external dependencies to a minimum
3. **Security**: Isolate clients and protect host system
4. **Modularity**: Pluggable backend system
5. **Protocol Fidelity**: Stay close to X11 protocol for efficiency
6. **Performance**: Minimize translation overhead

## Architecture Layers

```
┌─────────────────────────────────────┐
│         X11 Clients                 │
└───────────┬─────────────────────────┘
            │ (TCP/Unix Socket)
┌───────────▼─────────────────────────┐
│    Connection Layer                 │
│  - TCP listener                     │
│  - Unix socket listener             │
│  - Client authentication            │
└───────────┬─────────────────────────┘
            │
┌───────────▼─────────────────────────┐
│   Protocol Layer                    │
│  - Request parsing                  │
│  - Reply encoding                   │
│  - Event encoding                   │
│  - Error handling                   │
└───────────┬─────────────────────────┘
            │
┌───────────▼─────────────────────────┐
│     Core Server                     │
│  - Window tree management           │
│  - Resource tracking (GC, Pixmap)   │
│  - Event queue & dispatch           │
│  - Client session management        │
│  - Atom management                  │
│  - Selection management             │
└───────────┬─────────────────────────┘
            │
┌───────────▼─────────────────────────┐
│   Security/Isolation Layer          │
│  - Window tree isolation            │
│  - Property access control          │
│  - Selection access control         │
│  - Keyboard/pointer grab limits     │
└───────────┬─────────────────────────┘
            │
┌───────────▼─────────────────────────┐
│   Backend Trait                     │
│  - Window operations                │
│  - Graphics operations              │
│  - Input event handling             │
│  - Display configuration            │
└───────────┬─────────────────────────┘
            │
    ┌───────┴────────┬──────────┬─────────┐
    ▼                ▼          ▼         ▼
┌────────┐    ┌──────────┐ ┌────────┐ ┌────────┐
│  X11   │    │ Wayland  │ │ macOS  │ │Windows │
│Backend │    │ Backend  │ │Backend │ │Backend │
└────────┘    └──────────┘ └────────┘ └────────┘
```

## Module Structure

```
x11anywhere/
├── Cargo.toml
├── README.md
├── ARCHITECTURE.md
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library interface
│   │
│   ├── connection/          # Network layer
│   │   ├── mod.rs
│   │   ├── tcp.rs           # TCP socket handling
│   │   ├── unix.rs          # Unix socket handling
│   │   └── auth.rs          # X11 authentication
│   │
│   ├── protocol/            # X11 protocol implementation
│   │   ├── mod.rs
│   │   ├── types.rs         # Basic X11 types (Window, Pixmap, etc.)
│   │   ├── requests.rs      # X11 request definitions
│   │   ├── replies.rs       # X11 reply definitions
│   │   ├── events.rs        # X11 event definitions
│   │   ├── errors.rs        # X11 error definitions
│   │   ├── parser.rs        # Request parser
│   │   ├── encoder.rs       # Reply/event encoder
│   │   └── atoms.rs         # Atom management
│   │
│   ├── server/              # Core server logic
│   │   ├── mod.rs
│   │   ├── window.rs        # Window tree management
│   │   ├── drawable.rs      # Drawable abstraction
│   │   ├── gc.rs            # Graphics context
│   │   ├── pixmap.rs        # Pixmap management
│   │   ├── client.rs        # Client session
│   │   ├── resource.rs      # Resource ID management
│   │   ├── event_queue.rs   # Event queuing & dispatch
│   │   ├── selection.rs     # Selection/clipboard handling
│   │   └── dispatch.rs      # Request dispatcher
│   │
│   ├── security/            # Security & isolation
│   │   ├── mod.rs
│   │   ├── isolation.rs     # Window tree isolation
│   │   └── policy.rs        # Security policy
│   │
│   └── backend/             # Backend implementations
│       ├── mod.rs
│       ├── trait.rs         # Backend trait definition
│       ├── x11.rs           # X11 backend (nested)
│       ├── wayland.rs       # Wayland backend
│       ├── macos.rs         # macOS (Cocoa) backend
│       └── windows.rs       # Windows backend
```

## Core Components

### 1. Connection Layer

Handles network connections from X11 clients:
- TCP socket listener (configurable port, default 6000+)
- Unix socket listener (on Unix systems: /tmp/.X11-unix/X{n})
- X11 authentication (MIT-MAGIC-COOKIE-1, etc.)
- Byte order negotiation

### 2. Protocol Layer

Implements X11 wire protocol:
- Request parsing (80+ core requests)
- Reply encoding
- Event encoding (30+ event types)
- Error generation
- Extension mechanism (for future)

Key protocol elements:
- SetupRequest/SetupReply for connection initialization
- Request header parsing (major opcode, length, data)
- Reply/Error/Event encoding with proper sequencing

### 3. Core Server

Main server logic:
- **Window Tree**: Hierarchical window management, properties, attributes
- **Resource Management**: XIDs for windows, pixmaps, GCs, etc.
- **Graphics Context**: Drawing parameters (foreground, background, line width, etc.)
- **Pixmaps**: Off-screen drawables
- **Atoms**: String interning for property names
- **Selections**: Clipboard and drag-and-drop
- **Event Queue**: Per-client event queues with proper masking

### 4. Security Layer

Provides isolation and protection:
- **Window Isolation**: Clients only see their own windows by default
- **Property Protection**: Restrict property access between clients
- **Selection Control**: Mediate clipboard access
- **Grab Restrictions**: Limit keyboard/pointer grabs
- **Resource Limits**: Prevent resource exhaustion

### 5. Backend Abstraction

Trait-based system for platform integration:

```rust
trait Backend {
    // Window operations
    fn create_window(&mut self, params: WindowParams) -> Result<BackendWindow>;
    fn destroy_window(&mut self, window: BackendWindow) -> Result<()>;
    fn map_window(&mut self, window: BackendWindow) -> Result<()>;
    fn configure_window(&mut self, window: BackendWindow, config: WindowConfig) -> Result<()>;

    // Graphics operations
    fn draw_rectangle(&mut self, drawable: BackendDrawable, gc: &GC, rect: Rectangle) -> Result<()>;
    fn draw_text(&mut self, drawable: BackendDrawable, gc: &GC, x: i16, y: i16, text: &str) -> Result<()>;
    fn copy_area(&mut self, src: BackendDrawable, dst: BackendDrawable, gc: &GC, rect: Rectangle) -> Result<()>;

    // Event handling
    fn poll_events(&mut self) -> Vec<BackendEvent>;

    // Display info
    fn get_screen_info(&self) -> ScreenInfo;
}
```

### Backend Implementations

#### X11 Backend
- Uses X11 client library (xcb or xlib)
- Nested X server within existing X server
- Direct protocol mapping (most efficient)
- For testing on Linux/BSD

#### Wayland Backend
- Uses wayland-client
- Maps X11 windows to Wayland surfaces
- Handles compositor protocol differences
- For modern Linux

#### macOS Backend
- Uses Cocoa/AppKit via objc crate
- Maps to NSWindow
- Core Graphics for rendering
- Metal for acceleration (future)

#### Windows Backend
- Uses windows-sys or winapi
- Maps to Win32 HWND
- GDI/GDI+ for rendering
- Direct2D for acceleration (future)

## X11 Protocol Coverage

### Phase 1: Core Protocol
- Connection setup
- Window management (Create, Destroy, Map, Unmap, Configure)
- Basic drawing (PolyRectangle, PolyFillRectangle, PolyLine, PolyPoint)
- Graphics contexts
- Properties
- Events (Expose, ButtonPress, KeyPress, etc.)
- Atoms

### Phase 2: Essential Features
- Pixmaps
- Selections (clipboard)
- Keyboard/pointer grabs
- Fonts (initially with backend fonts)
- Images (PutImage, GetImage)

### Phase 3: Extensions
- RENDER (alpha blending, gradients)
- XFIXES (selection improvements)
- DAMAGE (efficient redrawing)
- COMPOSITE (compositing)

## Configuration

Support configuration via:
- Command line arguments
- Config file (TOML)
- Environment variables

Example config:
```toml
[server]
display = 1
tcp_port = 6001
unix_socket = true

[security]
window_isolation = true
allow_global_selections = false
allow_keyboard_grabs = false

[backend]
type = "x11"  # or "wayland", "macos", "windows"

[backend.x11]
display = ":0"
```

## Dependencies Strategy

Minimize dependencies:
- **No async runtime**: Use standard library threads
- **Minimal parsing**: Hand-written parsers for X11 protocol
- **Platform-specific only when needed**: Feature flags for backend deps

Core dependencies:
- `byteorder`: For protocol byte order handling
- `nix`: Unix socket and file descriptor handling (Unix only)
- `windows-sys`: Windows APIs (Windows only)
- `cocoa` / `core-graphics`: macOS APIs (macOS only)

## Future Enhancements

1. **Performance**: Zero-copy where possible, shared memory
2. **Extensions**: RENDER, XFIXES, DAMAGE, COMPOSITE, RANDR
3. **OpenGL**: GLX support for 3D applications
4. **Multi-monitor**: Proper multi-screen support
5. **Network transparency**: Compression, encryption
6. **Wayland protocols**: XWayland compatibility
