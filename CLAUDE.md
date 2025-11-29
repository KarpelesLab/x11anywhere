# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

X11Anywhere is a portable X11 display server written in Rust. It accepts X11 protocol connections and translates them to native display systems through a pluggable backend architecture. The server implements the core X11 protocol including window management, graphics operations, events, atoms, selections, and fonts.

## Build Commands

```bash
# Build with platform defaults (recommended)
cargo build

# Build release
cargo build --release

# Run with auto-detected backend
cargo run -- -display 1

# Run with specific backend
cargo run -- -display 1 -backend x11

# Run tests
cargo test

# Run visual regression test
cargo test --test visual_test -- --nocapture

# Check for errors
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Platform-Specific Builds

The build system auto-enables backends per platform:
- **Linux/BSD**: X11 + Wayland backends
- **macOS**: macOS native backend (Cocoa/Core Graphics via Swift FFI)
- **Windows**: Windows native backend (Win32/GDI)

For custom backend selection:
```bash
cargo build --no-default-features --features backend-x11
cargo build --no-default-features --features backend-wayland
```

## Architecture

### Layer Structure (top to bottom)

1. **Connection Layer** (`src/connection/`): TCP/Unix socket listeners, X11 authentication, byte order negotiation

2. **Protocol Layer** (`src/protocol/`): X11 wire protocol - request parsing, reply/event encoding, error handling

3. **Server Core** (`src/server/`): Window tree management, resource tracking (GC, Pixmap), event queues, client sessions, atoms, selections

4. **Security Layer** (`src/security/`): Window isolation, property/selection access control, grab restrictions

5. **Backend Abstraction** (`src/backend/`): Trait-based system translating X11 operations to native windowing

### Backend Implementation Pattern

All backends implement the `Backend` trait (`src/backend/trait.rs`) which defines:
- Window operations: create, destroy, map, unmap, configure, raise, lower
- Drawing operations: rectangles, lines, arcs, polygons, text, images
- Pixmap operations: create, free, copy
- Event polling: non-blocking and blocking variants
- Cursor operations: standard cursors from X11 cursor font
- Font operations: list system fonts, query metrics

Backend files:
- `src/backend/x11.rs` - Uses x11rb crate to connect to existing X server
- `src/backend/macos.rs` - FFI to Swift module (compiled separately)
- `src/backend/windows.rs` - Win32/GDI via windows-sys
- `src/backend/null.rs` - No-op backend for testing

### Key Data Flow

1. Client connects via TCP (port 6000+display) or Unix socket
2. `src/server/listener.rs` handles connection setup
3. `src/server/client.rs` manages per-client state and request handling
4. Requests are parsed by `src/protocol/parser.rs`
5. Server dispatches to appropriate handler which calls backend methods
6. Backend translates to native windowing system
7. Backend events are polled and translated back to X11 events

### Protocol Types

`src/protocol/types.rs` defines X11 primitives: Window, Pixmap, GContext, Atom, Cursor, Colormap, and related enums (WindowClass, StackMode, GCFunction, etc.)

### Resource Management

`src/resources/mod.rs` handles XID allocation and resource tracking for windows, pixmaps, graphics contexts, and other X11 resources.

## Visual Testing

Visual regression tests in `tests/` verify rendering correctness:
- Tests draw patterns (colors, shapes, arcs, text) via X11 protocol
- Screenshots are captured and compared against reference images
- Platform-specific screenshot capture: xwd/ImageMagick (Linux), screencapture (macOS), GDI (Windows)
- Reference images stored in `docs/screenshots/{platform}/`

## macOS Swift FFI

The macOS backend uses a Swift module for Cocoa/Core Graphics integration. The Swift code is compiled separately and provides a C API that Rust calls via FFI. No additional Rust dependencies are needed for macOS - the Swift Package provides everything.

## GitHub Actions Artifacts

When downloading artifacts from GitHub Actions using `gh run download`, use the `tmp` directory:
```bash
gh run download <run-id> -D tmp
```
