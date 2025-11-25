# X11Anywhere Implementation Status

This document tracks the implementation status of X11 protocol features across different backend platforms.

## Legend

- ✅ **Implemented**: Feature is fully implemented and tested
- 🟡 **Partial**: Feature is partially implemented or has limitations
- ❌ **Not Implemented**: Feature exists as stub/placeholder only
- ⚪ **Not Applicable**: Feature doesn't apply to this backend

## Backends Overview

| Backend | Status | Priority | Notes |
|---------|--------|----------|-------|
| X11 (Linux/BSD) | ✅ Implemented | High | All drawing ops working, visual tests passing |
| Windows | ✅ Implemented | High | Full Win32/GDI implementation complete, **compiles & passes CI** |
| macOS | ✅ Implemented | High | Swift FFI implementation complete for both ARM64 & x86_64, **compiles & passes CI** |
| Wayland | ❌ Not Started | Medium | Planned for future |

---

## Core Protocol Features

### Connection & Setup

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| Connection establishment | ✅ | ✅ | ✅ | ⚪ | X11 socket connection working; Windows/macOS via init() |
| Authentication | ✅ | ⚪ | ⚪ | ⚪ | Basic auth implemented; N/A for native backends |
| Screen info | ✅ | ✅ | ✅ | ⚪ | All backends return screen dimensions, visuals |
| Extension querying | ✅ | ⚪ | ⚪ | ⚪ | X11 only; extensions N/A for native backends |

### Window Management

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| CreateWindow | ✅ | ✅ | ✅ | ⚪ | CreateWindowExW on Windows, NSWindow on macOS |
| DestroyWindow | ✅ | ✅ | ✅ | ⚪ | DestroyWindow on Windows, close on macOS |
| MapWindow (show) | ✅ | ✅ | ✅ | ⚪ | ShowWindow on Windows, makeKeyAndOrderFront on macOS |
| UnmapWindow (hide) | ✅ | ✅ | ✅ | ⚪ | ShowWindow(SW_HIDE) on Windows, orderOut on macOS |
| ConfigureWindow | ✅ | ✅ | ✅ | ⚪ | SetWindowPos on Windows, setFrame on macOS |
| ReparentWindow | 🟡 | ❌ | ❌ | ⚪ | May have limitations on native platforms |
| ChangeWindowAttributes | 🟡 | ❌ | ❌ | ⚪ | Partial support |
| GetWindowAttributes | ✅ | ✅ | ✅ | ⚪ | Server-side; returns default window attributes |
| GetGeometry | ✅ | ✅ | ✅ | ⚪ | Server-side; returns window/drawable geometry |
| QueryTree | ✅ | ✅ | ✅ | ⚪ | Server-side; returns window hierarchy |
| RaiseWindow | ✅ | ✅ | ✅ | ⚪ | SetWindowPos(HWND_TOP) on Windows, orderFront on macOS |
| LowerWindow | ✅ | ✅ | ✅ | ⚪ | SetWindowPos(HWND_BOTTOM) on Windows, orderBack on macOS |

### Drawing Operations

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| ClearArea | ✅ | ✅ | ✅ | ⚪ | FillRect on Windows, fillRect on macOS |
| PolyPoint | ✅ | ✅ | ✅ | ⚪ | SetPixel on Windows, 1x1 rects on macOS |
| PolyLine | ✅ | ✅ | ✅ | ⚪ | LineTo on Windows, CGContext paths on macOS |
| PolySegment | ✅ | ✅ | ✅ | ⚪ | Multiple LineTo calls |
| PolyRectangle | ✅ | ✅ | ✅ | ⚪ | Rectangle on Windows, stroke_rect on macOS |
| PolyFillRectangle | ✅ | ✅ | ✅ | ⚪ | FillRect on Windows, fill_rect on macOS |
| FillPoly | ✅ | ✅ | ✅ | ⚪ | Polygon on Windows, CGContext paths on macOS |
| PolyArc | ✅ | ✅ | ✅ | ⚪ | Arc/Pie on Windows, CGContext ellipse transforms on macOS |
| PolyFillArc | ✅ | ✅ | ✅ | ⚪ | Pie on Windows, CGContext arcs on macOS |
| CopyArea | ✅ | ✅ | ✅ | ⚪ | BitBlt on Windows; CGImage cropping/drawing on macOS |
| ImageText8 | ✅ | ✅ | ✅ | ⚪ | TextOutW on Windows, Core Text (CTLineDraw) on macOS |
| ImageText16 | ✅ | ✅ | ✅ | ⚪ | Unicode text rendering supported |
| PutImage | ✅ | ✅ | ✅ | ⚪ | SetDIBitsToDevice on Windows, CGImage on macOS |
| GetImage | ✅ | ✅ | ✅ | ⚪ | GetDIBits on Windows, CGContext.makeImage on macOS |

### Graphics Context (GC)

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| CreateGC | ✅ | 🟡 | 🟡 | ⚪ | GC tracked in BackendGC struct; pen/brush created per draw |
| ChangeGC | 🟡 | 🟡 | 🟡 | ⚪ | GC state tracked; applied during drawing operations |
| FreeGC | ✅ | ✅ | ✅ | ⚪ | GC cleanup handled per operation |
| SetForeground | 🟡 | ✅ | ✅ | ⚪ | Applied via create_pen/create_brush; CGColor on macOS |
| SetBackground | 🟡 | ✅ | ✅ | ⚪ | Applied during drawing operations |
| SetLineWidth | 🟡 | ✅ | ✅ | ⚪ | CreatePen with width on Windows; line_width on macOS |
| SetLineStyle | 🟡 | 🟡 | 🟡 | ⚪ | Basic line styles supported |
| SetFunction | 🟡 | ❌ | ❌ | ⚪ | Raster operations not fully implemented |

### Pixmaps (Off-screen Drawables)

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| CreatePixmap | 🟡 | ✅ | ✅ | ⚪ | CreateCompatibleDC/Bitmap on Windows, CGContext on macOS |
| FreePixmap | 🟡 | ✅ | ✅ | ⚪ | DeleteDC/DeleteObject on Windows; CGContext release on macOS |
| Draw to pixmap | 🟡 | ✅ | ✅ | ⚪ | All drawing operations work on pixmaps |
| Copy pixmap to window | 🟡 | ✅ | 🟡 | ⚪ | BitBlt on Windows; macOS needs improvement |

### Color & Colormaps

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| AllocColor | ✅ | ✅ | ✅ | ⚪ | Server-side RGB to pixel conversion (TrueColor) |
| AllocNamedColor | ✅ | ✅ | ✅ | ⚪ | Server-side named color lookup (70+ colors) |
| FreeColors | 🟡 | ✅ | ✅ | ⚪ | N/A for TrueColor (no-op) |
| CreateColormap | 🟡 | ✅ | ✅ | ⚪ | Limited support (TrueColor only) |
| FreeColormap | 🟡 | ✅ | ✅ | ⚪ | N/A for TrueColor (no-op) |

### Fonts

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| OpenFont | ✅ | ✅ | ✅ | ⚪ | Server-side font tracking with FontInfo struct |
| CloseFont | ✅ | ✅ | ✅ | ⚪ | Server-side font tracking |
| QueryFont | ✅ | ✅ | ✅ | ⚪ | Server-side; returns font metrics (ascent, descent, char width) |
| ListFonts | ✅ | ✅ | ✅ | ⚪ | Server-side; returns built-in font names matching pattern |

### Events

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| Expose | 🟡 | ✅ | ✅ | ⚪ | WM_PAINT on Windows; NSView drawRect on macOS |
| ConfigureNotify | 🟡 | ✅ | ✅ | ⚪ | WM_SIZE on Windows; NSWindow resize on macOS |
| MapNotify | 🟡 | ✅ | ✅ | ⚪ | Generated when map_window() is called |
| UnmapNotify | 🟡 | ✅ | ✅ | ⚪ | Generated when unmap_window() is called |
| DestroyNotify | 🟡 | ✅ | ✅ | ⚪ | WM_CLOSE on Windows; NSWindow close on macOS |
| KeyPress | 🟡 | ✅ | ✅ | ⚪ | WM_KEYDOWN on Windows; NSEvent keyDown on macOS |
| KeyRelease | 🟡 | ✅ | ✅ | ⚪ | WM_KEYUP on Windows; NSEvent keyUp on macOS |
| ButtonPress | 🟡 | ✅ | ✅ | ⚪ | WM_LBUTTONDOWN/etc on Windows; NSEvent mouseDown on macOS |
| ButtonRelease | 🟡 | ✅ | ✅ | ⚪ | WM_LBUTTONUP/etc on Windows; NSEvent mouseUp on macOS |
| MotionNotify | 🟡 | ✅ | ✅ | ⚪ | WM_MOUSEMOVE on Windows; NSEvent mouseMoved on macOS |
| EnterNotify | 🟡 | ✅ | ✅ | ⚪ | TrackMouseEvent on Windows; mouseEntered on macOS with NSTrackingArea |
| LeaveNotify | 🟡 | ✅ | ✅ | ⚪ | WM_MOUSELEAVE on Windows; mouseExited on macOS with NSTrackingArea |
| FocusIn | 🟡 | ✅ | ✅ | ⚪ | WM_SETFOCUS on Windows; NSWindow becomeKey on macOS |
| FocusOut | 🟡 | ✅ | ✅ | ⚪ | WM_KILLFOCUS on Windows; NSWindow resignKey on macOS |

### Input

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| GrabKeyboard | 🟡 | ❌ | ❌ | ⚪ | SetCapture on Windows (limited) |
| UngrabKeyboard | 🟡 | ❌ | ❌ | ⚪ | ReleaseCapture on Windows |
| GrabPointer | 🟡 | ❌ | ❌ | ⚪ | SetCapture on Windows |
| UngrabPointer | 🟡 | ❌ | ❌ | ⚪ | ReleaseCapture on Windows |
| SetInputFocus | 🟡 | ❌ | ❌ | ⚪ | SetFocus on Windows, makeKeyWindow on macOS |
| GetInputFocus | 🟡 | ❌ | ❌ | ⚪ | GetFocus on Windows |
| QueryPointer | 🟡 | ❌ | ❌ | ⚪ | GetCursorPos on Windows |
| WarpPointer | 🟡 | ❌ | ❌ | ⚪ | SetCursorPos on Windows |

### Properties & Atoms

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| InternAtom | ✅ | ✅ | ✅ | ⚪ | String-to-ID mapping (server-side) |
| GetAtomName | ✅ | ✅ | ✅ | ⚪ | ID-to-string lookup (server-side) |
| ChangeProperty | ✅ | ✅ | ✅ | ⚪ | Window properties storage (server-side) |
| DeleteProperty | ✅ | ✅ | ✅ | ⚪ | Server-side property storage |
| GetProperty | ✅ | ✅ | ✅ | ⚪ | Server-side property storage |
| ListProperties | ✅ | ✅ | ✅ | ⚪ | Server-side property storage |

### Selections (Clipboard)

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| SetSelectionOwner | ✅ | ✅ | ✅ | ⚪ | Server-side selection tracking |
| GetSelectionOwner | ✅ | ✅ | ✅ | ⚪ | Server-side selection tracking |
| ConvertSelection | 🟡 | 🟡 | 🟡 | ⚪ | Parsed; needs full conversion protocol |

### Cursors

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| CreateCursor | 🟡 | 🟡 | 🟡 | ⚪ | System cursors via LoadCursorW on Windows, NSCursor on macOS |
| FreeCursor | 🟡 | ✅ | ✅ | ⚪ | System cursors don't need freeing |
| DefineCursor | 🟡 | ✅ | ✅ | ⚪ | SetCursor on Windows, NSCursor.set on macOS |
| CreateGlyphCursor | 🟡 | ✅ | ✅ | ⚪ | Maps X11 cursor font glyphs to system cursors |

### Extensions

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| QueryExtension | ✅ | ✅ | ✅ | ⚪ | Returns extension info from server registry |
| ListExtensions | ✅ | ✅ | ✅ | ⚪ | Lists all registered extensions |

### Miscellaneous

| Feature | X11 | Windows | macOS | Wayland | Notes |
|---------|-----|---------|-------|---------|-------|
| Bell | ✅ | ✅ | ✅ | ⚪ | No-op; could use platform beep APIs |
| GetInputFocus | ✅ | ✅ | ✅ | ⚪ | Returns focus window |
| SetInputFocus | 🟡 | 🟡 | 🟡 | ⚪ | Partially implemented |

### X11 Extensions Status

| Extension | Status | Notes |
|-----------|--------|-------|
| BIG-REQUESTS | 🟡 Registered | Allows requests > 256KB; registered but not fully implemented |
| XKEYBOARD (XKB) | 🟡 Registered | Advanced keyboard; registered but not implemented |
| RENDER | ❌ Not Implemented | Anti-aliased rendering, gradients, alpha blending |
| XFIXES | ❌ Not Implemented | Cursor visibility, region support |
| DAMAGE | ❌ Not Implemented | Tracks drawable changes |
| COMPOSITE | ❌ Not Implemented | Off-screen window rendering |
| SHAPE | ❌ Not Implemented | Non-rectangular windows |
| SYNC | ❌ Not Implemented | Synchronization primitives |
| RANDR | ❌ Not Implemented | Screen configuration |
| Xinerama | ❌ Not Implemented | Multi-monitor support |
| GLX | ❌ Not Implemented | OpenGL integration |
| MIT-SHM | ❌ Not Implemented | Shared memory for images |

---

## Platform-Specific Implementation Notes

### X11 Backend (Linux/BSD)
- **Status**: ✅ **Fully implemented** - basic passthrough working via direct X11 protocol
- **Architecture**: Direct protocol translation to underlying X11 server
- **Working Features**:
  - ✅ Window management (CreateWindow, MapWindow, UnmapWindow, DestroyWindow, ConfigureWindow)
  - ✅ GC operations (CreateGC, ChangeGC)
  - ✅ PolyFillRectangle (opcode 70)
  - ✅ PolyRectangle (opcode 67)
  - ✅ PolyLine via PolySegment (opcode 66)
  - ✅ PolyPoint (opcode 64)
  - ✅ PolyArc (opcode 68)
  - ✅ PolyFillArc (opcode 71)
  - ✅ FillPoly (opcode 69)
  - ✅ PutImage (opcode 72)
  - ✅ GetImage (opcode 73)
  - ✅ CopyArea (opcode 62)
  - ✅ ImageText8 (opcode 76)
  - ✅ OpenFont (opcode 45)
  - ✅ CloseFont (opcode 46)
- **Not Yet Implemented**:
  - ❌ QueryFont
  - ❌ ListFonts
  - ❌ Event delivery (MapNotify, UnmapNotify, etc.)
- **Limitations**:
  - Some advanced extensions not implemented
  - Limited error handling
  - Event delivery to clients not yet implemented
- **Next Steps**: Implement event delivery infrastructure

### Windows Backend
- **Status**: ✅ **Fully implemented** (visual tests passing)
- **Architecture**: X11 protocol → Win32 API translation
- **Implemented APIs**:
  - Window management: `CreateWindowExW`, `ShowWindow`, `SetWindowPos`, `DestroyWindow`
  - Drawing: GDI (`Rectangle`, `FillRect`, `TextOutW`, `LineTo`, `SetPixel`, `BitBlt`, `Arc`, `Pie`, `Polygon`)
  - Resources: `CreatePen`, `CreateSolidBrush`, `CreateCompatibleDC/Bitmap`
  - Events: Windows message loop (`PeekMessageW`, `GetMessageW`, `DispatchMessageW`)
  - Supported events: WM_PAINT, WM_SIZE, WM_CLOSE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSELEAVE (for EnterNotify/LeaveNotify via TrackMouseEvent), WM_SETFOCUS, WM_KILLFOCUS
- **Working Features**:
  - ✅ Window creation, mapping, configuration, raising/lowering
  - ✅ Basic drawing: rectangles, lines, points, text
  - ✅ Arc and polygon drawing (Arc, Pie, Polygon GDI functions)
  - ✅ Image operations (SetDIBitsToDevice for PutImage, GetDIBits for GetImage)
  - ✅ Pixmaps (off-screen drawing with compatible DCs)
  - ✅ Enhanced event handling: KeyPress/Release, ButtonPress/Release, MotionNotify, EnterNotify/LeaveNotify, FocusIn/Out
  - ✅ Event polling and blocking wait
  - ✅ GC state tracking (foreground, background, line width/style)
  - ✅ Cursor support: standard system cursors via LoadCursorW, WM_SETCURSOR handling
- **Known Limitations**:
  - No advanced raster operations (SetROP2)
  - Custom bitmap cursors not yet supported
- **Next Steps**: Test with real X11 applications

### macOS Backend
- **Status**: ✅ **Fully implemented** with Swift FFI (compiles & passes CI for ARM64 and x86_64)
- **Architecture**: X11 protocol → Swift module → Cocoa/Core Graphics
- **Implementation Approach**:
  - ✅ **Swift C API Module**: Created `swift/Sources/X11AnywhereBackend/MacOSBackend.swift` with native Cocoa/Core Graphics access
  - ✅ **FFI Bridge**: Rust backend (`src/backend/macos.rs`) calls Swift functions via C FFI (`@_cdecl`)
  - ✅ **Thread Safety**: Swift module handles all Cocoa objects on main thread using `DispatchQueue.main.sync`, Rust only holds opaque pointer
  - ✅ **Cross-Compilation**: Build system properly compiles Swift for both ARM64 and x86_64 architectures using target triples
- **Implemented Features**:
  - Window management: `NSWindow`, `NSApplication` with proper lifecycle management
  - Drawing: Core Graphics (`CGContext`) with native APIs (`stroke`, `fill`, `setStrokeColor`, etc.)
  - Arc and polygon drawing: CGContext ellipse transforms, path-based polygon filling
  - Image operations: CGImage for PutImage, CGContext.makeImage for GetImage
  - Resources: CGContext-based bitmap contexts for pixmaps
  - Events: Cocoa event loop with `NSApp.nextEvent`
  - Enhanced event handling: KeyPress/Release, ButtonPress/Release, MotionNotify, EnterNotify/LeaveNotify (via NSTrackingArea), FocusIn/Out, DestroyNotify
  - Supported operations: rectangles, lines, points, arcs, polygons, text, images, clear area, copy area (basic)
  - GC state tracking: foreground/background colors, line width
  - Cursor support: standard system cursors via NSCursor
- **Build System**:
  - Swift Package Manager integration via `build.rs`
  - Automatic SDK path detection with `xcrun`
  - Runtime library search paths via rpath
  - Proper linkage of Cocoa, Foundation, CoreGraphics, AppKit frameworks
- **Coordinate System**:
  - macOS CGContext uses bottom-left origin; X11 uses top-left
  - ✅ Handled via CTM transform (`translateBy`/`scaleBy`) in X11BackingBuffer context creation
  - All drawing operations use X11 coordinates directly; transform applied at context level
- **Known Limitations**:
  - Custom bitmap cursors not yet supported
- **Next Steps**: Test with real X11 applications

### Wayland Backend
- **Status**: Not started
- **Architecture**: X11 protocol → Wayland protocol translation
- **Key Considerations**:
  - No server-side window management
  - Compositor-specific features
  - Different security model
- **Timeline**: Future milestone

---

## Testing Status

| Backend | Unit Tests | Integration Tests | Visual Tests | Manual Testing | Notes |
|---------|------------|-------------------|--------------|----------------|-------|
| X11 | 🟡 Basic | 🟡 xcalc works | ✅ Passing | ✅ | All drawing ops working; visual tests validate all shapes |
| Windows | ❌ | ❌ | ✅ Passing | ⏳ Pending | All drawing ops working correctly |
| macOS | ❌ | ❌ | ✅ Passing | ⏳ Pending | All drawing ops working correctly |
| Wayland | ❌ | ❌ | ❌ | ❌ | Not started |

### Visual Test Coverage
The visual test (`tests/visual_test.rs`) validates the following operations:
- ✅ PolyFillRectangle (opcode 70) - 6 colored rectangles
- ✅ PolyLine (opcode 65) - zigzag pattern
- ✅ PolyRectangle (opcode 67) - rectangle outlines
- ✅ PolyArc (opcode 68) - semicircle outline
- ✅ PolyFillArc (opcode 71) - pie slice
- ✅ FillPoly (opcode 69) - triangle
- ✅ PolyPoint (opcode 64) - dot grid
- ✅ PolySegment (opcode 66) - X shape
- ✅ OpenFont (opcode 45) - font loading
- ✅ ImageText8 (opcode 76) - text rendering

---

## Priority Roadmap

### Phase 1: Core Window Management ✅ **COMPLETED**
- [x] Create backend stubs for Windows and macOS
- [x] **Windows**: Implement window creation, mapping, configuration
- [x] **macOS**: Implement window creation, mapping, configuration
- [x] **Windows**: Implement basic event handling (expose, configure, mouse, keyboard)
- [x] **macOS**: Implement basic event handling (framework in place)

### Phase 2: Basic Drawing ✅ **COMPLETED**
- [x] **Windows**: Implement GDI drawing operations (rectangles, lines, text)
- [x] **macOS**: Implement Core Graphics drawing operations
- [x] **Both**: Implement pixmap support
- [ ] **Both**: Test with simple X11 applications ⏳ **IN PROGRESS**

### Phase 3: Advanced Features ✅ **COMPLETED**
- [x] **Both**: Enhanced event handling (ButtonRelease, MotionNotify, Focus events) ✅
- [x] **Both**: Arc and polygon drawing operations ✅
- [x] **Both**: Image operations (PutImage, GetImage) ✅
- [x] **macOS**: Improve copy_area() with proper CGImage implementation ✅
- [x] **Both**: Cursor support (standard system cursors) ✅
- [x] **Both**: Window property operations (server-side storage) ✅
- [x] **Both**: Selection/clipboard support (server-side tracking) ✅
- [x] **Both**: Advanced font handling (QueryFont, ListFonts) ✅
- [x] **Both**: Advanced color management (AllocColor, AllocNamedColor) ✅

### Phase 4: Optimization & Testing (Current Phase)
- [ ] Performance profiling
- [ ] Comprehensive testing with various X11 applications
- [ ] Bug fixes and edge cases
- [ ] Documentation

### Phase 5: Wayland Support
- [ ] Research and design Wayland backend
- [ ] Implement basic Wayland support
- [ ] Test on various compositors

---

## Known Limitations

### Cross-Platform Challenges

1. **Window Hierarchy**
   - X11: Flexible parent/child relationships
   - Windows/macOS: More restricted hierarchies
   - **Impact**: May need to virtualize some hierarchy operations

2. **Coordinate Systems**
   - X11: Origin at top-left, Y increases downward
   - macOS: Origin at bottom-left, Y increases upward
   - **Status**: ✅ Handled via CTM transform in X11BackingBuffer context creation

3. **Event Delivery**
   - X11: Server-side event filtering
   - Windows/macOS: OS-controlled event routing
   - **Impact**: Need to emulate X11 event masks

4. **Colormaps**
   - X11: Flexible colormap system
   - Modern systems: TrueColor assumed
   - **Impact**: Simplify to TrueColor only

5. **Window Grabbing**
   - X11: Server-enforced grabs
   - Windows/macOS: Limited grab support
   - **Impact**: May not support all grab scenarios

---

## Contributing

When implementing new features:
1. Update this document with implementation status
2. Add platform-specific notes and limitations
3. Document Win32/Cocoa API mappings
4. Add test cases for verification

## References

- [X11 Protocol Specification](https://www.x.org/releases/current/doc/xproto/x11protocol.html)
- [Win32 API Documentation](https://docs.microsoft.com/en-us/windows/win32/api/)
- [Cocoa Framework Documentation](https://developer.apple.com/documentation/appkit)
- [Wayland Protocol](https://wayland.freedesktop.org/docs/html/)
