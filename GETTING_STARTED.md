# Getting Started with X11Anywhere Development

This guide will help you understand the project structure and start contributing to X11Anywhere.

## Project Status

The project structure is complete and compiles successfully. Here's what has been implemented:

### ✅ Completed

- **Architecture Design**: Comprehensive architecture documented in [ARCHITECTURE.md](ARCHITECTURE.md)
- **Protocol Types**: Core X11 types (Window, Pixmap, GC, Atom, etc.) in `src/protocol/types.rs`
- **Protocol Events**: All major X11 events with encoding in `src/protocol/events.rs`
- **Protocol Errors**: X11 error codes and error handling in `src/protocol/errors.rs`
- **Request Definitions**: Request opcodes and structures in `src/protocol/requests.rs`
- **Backend Trait**: Abstract interface for display backends in `src/backend/trait.rs`
- **Connection Layer**: TCP and Unix socket support in `src/connection/mod.rs`
- **Security Policy**: Configurable security policies in `src/security/mod.rs`
- **Server Structure**: Basic server structure in `src/server/mod.rs`
- **CLI Interface**: Full command-line interface in `src/main.rs`

### 🚧 To Be Implemented

The following components need implementation:

1. **Protocol Parser** (Priority: High)
   - Parse incoming X11 requests from byte stream
   - Handle connection setup (SetupRequest/SetupReply)
   - Request validation and parsing

2. **Protocol Encoder** (Priority: High)
   - Encode replies to requests
   - Encode events to send to clients
   - Encode errors

3. **Core Server Components** (Priority: High)
   - Window tree management
   - Resource tracking (XIDs)
   - Graphics context management
   - Event queue and dispatch
   - Atom table management

4. **Backend Implementations** (Priority: Medium)
   - X11 backend (for testing on Linux)
   - Wayland backend
   - macOS backend
   - Windows backend

5. **Request Handlers** (Priority: Medium)
   - CreateWindow, DestroyWindow
   - MapWindow, UnmapWindow
   - ConfigureWindow
   - ChangeProperty, GetProperty
   - Drawing requests (PolyRectangle, etc.)

## Development Workflow

### Building

```bash
# Check compilation
cargo check

# Build debug version
cargo build

# Build release version
cargo build --release

# Build with X11 backend feature
cargo build --features backend-x11

# Run tests
cargo test
```

### Running

```bash
# Show help
./target/debug/x11anywhere --help

# List available backends
./target/debug/x11anywhere -list-backends

# Run (when implemented)
./target/debug/x11anywhere -display 1 -backend x11
```

### Code Organization

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library root
├── protocol/            # X11 protocol implementation
│   ├── types.rs         # Core types (Window, Pixmap, etc.)
│   ├── events.rs        # Event definitions and encoding
│   ├── errors.rs        # Error codes and handling
│   ├── requests.rs      # Request opcodes and structures
│   └── mod.rs           # Protocol module
├── backend/             # Backend abstraction
│   ├── trait.rs         # Backend trait definition
│   ├── x11.rs           # X11 backend (to be implemented)
│   ├── wayland.rs       # Wayland backend (to be implemented)
│   ├── macos.rs         # macOS backend (to be implemented)
│   ├── windows.rs       # Windows backend (to be implemented)
│   └── mod.rs           # Backend module
├── server/              # Core server logic
│   └── mod.rs           # Server implementation
├── connection/          # Network layer
│   └── mod.rs           # Connection handling
└── security/            # Security and isolation
    └── mod.rs           # Security policies
```

## Next Steps for Development

### 1. Implement Protocol Parser (Start Here)

Create `src/protocol/parser.rs`:

```rust
use super::*;
use std::io::Read;

pub struct ProtocolParser {
    // Connection byte order
    byte_order: ByteOrder,
}

impl ProtocolParser {
    pub fn new(byte_order: ByteOrder) -> Self {
        ProtocolParser { byte_order }
    }

    pub fn parse_setup_request(&mut self, stream: &mut dyn Read) -> Result<SetupRequest, X11Error> {
        // Parse connection setup
        todo!()
    }

    pub fn parse_request(&mut self, buffer: &[u8]) -> Result<Request, X11Error> {
        // Parse X11 request from buffer
        todo!()
    }
}
```

### 2. Implement Protocol Encoder

Create `src/protocol/encoder.rs`:

```rust
use super::*;

pub struct ProtocolEncoder {
    byte_order: ByteOrder,
}

impl ProtocolEncoder {
    pub fn encode_setup_reply(&self, reply: &SetupReply) -> Vec<u8> {
        // Encode setup reply
        todo!()
    }

    pub fn encode_reply(&self, reply: &Reply) -> Vec<u8> {
        // Encode reply to request
        todo!()
    }
}
```

### 3. Expand Server Implementation

Enhance `src/server/mod.rs`:

- Implement window tree management
- Add resource tracking
- Implement event queue
- Add request dispatcher

### 4. Implement X11 Backend

Create `src/backend/x11.rs`:

```rust
#[cfg(feature = "backend-x11")]
use x11rb::connection::Connection;

pub struct X11Backend {
    // X11 connection
    // Window mapping
}

impl Backend for X11Backend {
    fn init(&mut self) -> BackendResult<()> {
        // Connect to X11 display
        todo!()
    }

    fn create_window(&mut self, params: WindowParams) -> BackendResult<BackendWindow> {
        // Create X11 window
        todo!()
    }

    // ... implement other methods
}
```

## Testing Strategy

### Unit Tests

Add tests to each module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_creation() {
        let window = Window::new(123);
        assert_eq!(window.id().get(), 123);
    }
}
```

### Integration Tests

Create `tests/integration_test.rs`:

```rust
use x11anywhere::*;

#[test]
fn test_server_creation() {
    // Test server creation with mock backend
}
```

### Manual Testing

Use real X11 clients to test:

```bash
# Start server
./target/debug/x11anywhere -display 99 -backend x11

# In another terminal
export DISPLAY=:99
xterm  # Test with xterm
xclock # Test with xclock
```

## Debugging Tips

### Enable Logging

```bash
# Set log level
export RUST_LOG=debug
./target/debug/x11anywhere -display 1

# More verbose
export RUST_LOG=trace
```

### Use lldb/gdb

```bash
# Debug with lldb
lldb ./target/debug/x11anywhere
(lldb) run -display 1

# Or with gdb
gdb ./target/debug/x11anywhere
(gdb) run -display 1
```

### X11 Protocol Debugging

```bash
# Use xtrace to see X11 protocol
xtrace -D :99 xterm

# Or use Xephyr for nested testing
Xephyr :99 -screen 1024x768
```

## Contributing Guidelines

1. **Follow Rust conventions**: Use `rustfmt` and `clippy`
2. **Write tests**: Add tests for new functionality
3. **Document**: Add doc comments to public APIs
4. **Keep it modular**: Maintain separation between protocol, server, and backends
5. **Minimal dependencies**: Only add dependencies when necessary

## Resources

### X11 Protocol References

- [X Window System Protocol](https://www.x.org/releases/X11R7.7/doc/xproto/x11protocol.html)
- [Xlib Manual](https://tronche.com/gui/x/xlib/)
- [XCB Documentation](https://xcb.freedesktop.org/)

### Rust X11 Libraries

- [x11rb](https://docs.rs/x11rb/) - X11 Rust bindings
- [xcb](https://docs.rs/xcb/) - X11 XCB bindings

### Similar Projects

- [Xwayland](https://wayland.freedesktop.org/xserver.html)
- [Xephyr](https://www.freedesktop.org/wiki/Software/Xephyr/)
- [TinyWM](http://incise.org/tinywm.html) - Minimal window manager

## Questions?

If you have questions or need help:

1. Check [ARCHITECTURE.md](ARCHITECTURE.md) for design details
2. Review the code comments
3. Look at X11 protocol documentation
4. Open an issue for discussion

Happy coding!
