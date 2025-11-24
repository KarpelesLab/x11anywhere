// MacOSBackend.swift - Swift wrapper for macOS Cocoa/Core Graphics operations
// Exposes C API for Rust FFI

import Cocoa
import Foundation

// MARK: - C API Types

/// Opaque handle to a Swift backend instance
public typealias BackendHandle = UnsafeMutableRawPointer

/// Opaque handle to a window
public typealias WindowHandle = UnsafeMutableRawPointer

/// Opaque handle to a graphics context (pixmap)
public typealias ContextHandle = UnsafeMutableRawPointer

/// Result codes
public enum BackendResult: Int32 {
    case success = 0
    case error = 1
}

// MARK: - Backend Class

class MacOSBackendImpl {
    var windows: [Int: NSWindow] = [:]
    var contexts: [Int: CGContext] = [:]
    var nextId: Int = 1
    var screenWidth: Int = 0
    var screenHeight: Int = 0
    var screenWidthMM: Int = 0
    var screenHeightMM: Int = 0

    init() {
        // Ensure we're on main thread for Cocoa operations
        DispatchQueue.main.sync {
            // Initialize NSApplication
            let app = NSApplication.shared
            app.setActivationPolicy(.regular)

            // Get screen dimensions
            if let screen = NSScreen.main {
                let frame = screen.frame
                self.screenWidth = Int(frame.width)
                self.screenHeight = Int(frame.height)

                // Estimate physical size (72 DPI base * backing scale)
                let backingScale = screen.backingScaleFactor
                let dpi = 72.0 * backingScale
                self.screenWidthMM = Int(Double(self.screenWidth) * 25.4 / dpi)
                self.screenHeightMM = Int(Double(self.screenHeight) * 25.4 / dpi)
            }
        }
    }

    func createWindow(x: Int, y: Int, width: Int, height: Int) -> Int {
        var windowId = 0
        DispatchQueue.main.sync {
            let id = self.nextId
            self.nextId += 1

            // Convert Y coordinate (X11 top-left to macOS bottom-left)
            let flippedY = self.screenHeight - y - height

            let rect = NSRect(x: CGFloat(x), y: CGFloat(flippedY),
                            width: CGFloat(width), height: CGFloat(height))

            let styleMask: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable]
            let window = NSWindow(contentRect: rect,
                                styleMask: styleMask,
                                backing: .buffered,
                                defer: false)

            window.title = "X11 Window"
            window.backgroundColor = .white

            self.windows[id] = window
            windowId = id
        }
        return windowId
    }

    func destroyWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows.removeValue(forKey: id) {
                window.close()
            }
        }
    }

    func mapWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                window.makeKeyAndOrderFront(nil)
            }
        }
    }

    func unmapWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                window.orderOut(nil)
            }
        }
    }

    func configureWindow(id: Int, x: Int, y: Int, width: Int, height: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                let flippedY = self.screenHeight - y - height
                let rect = NSRect(x: CGFloat(x), y: CGFloat(flippedY),
                                width: CGFloat(width), height: CGFloat(height))
                window.setFrame(rect, display: true)
            }
        }
    }

    func raiseWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                window.orderFront(nil)
            }
        }
    }

    func lowerWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                window.orderBack(nil)
            }
        }
    }

    func setWindowTitle(id: Int, title: String) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                window.title = title
            }
        }
    }

    func getWindowContext(id: Int) -> CGContext? {
        var context: CGContext? = nil
        DispatchQueue.main.sync {
            if let window = self.windows[id],
               let contentView = window.contentView {
                // Lock focus to get graphics context
                contentView.lockFocus()
                context = NSGraphicsContext.current?.cgContext
            }
        }
        return context
    }

    func releaseWindowContext(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id],
               let contentView = window.contentView {
                contentView.unlockFocus()
            }
        }
    }

    func createPixmap(width: Int, height: Int) -> Int {
        let id = self.nextId
        self.nextId += 1

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue

        if let context = CGContext(data: nil,
                                   width: width,
                                   height: height,
                                   bitsPerComponent: 8,
                                   bytesPerRow: width * 4,
                                   space: colorSpace,
                                   bitmapInfo: bitmapInfo) {
            self.contexts[id] = context
            return id
        }
        return 0
    }

    func getPixmapContext(id: Int) -> CGContext? {
        return self.contexts[id]
    }

    func freePixmap(id: Int) {
        self.contexts.removeValue(forKey: id)
    }
}

// MARK: - C API Implementation

@_cdecl("macos_backend_create")
public func macos_backend_create() -> BackendHandle {
    let backend = MacOSBackendImpl()
    return Unmanaged.passRetained(backend).toOpaque()
}

@_cdecl("macos_backend_destroy")
public func macos_backend_destroy(_ handle: BackendHandle) {
    Unmanaged<MacOSBackendImpl>.fromOpaque(handle).release()
}

@_cdecl("macos_backend_get_screen_info")
public func macos_backend_get_screen_info(_ handle: BackendHandle,
                                         width: UnsafeMutablePointer<Int32>,
                                         height: UnsafeMutablePointer<Int32>,
                                         widthMM: UnsafeMutablePointer<Int32>,
                                         heightMM: UnsafeMutablePointer<Int32>) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    width.pointee = Int32(backend.screenWidth)
    height.pointee = Int32(backend.screenHeight)
    widthMM.pointee = Int32(backend.screenWidthMM)
    heightMM.pointee = Int32(backend.screenHeightMM)
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_create_window")
public func macos_backend_create_window(_ handle: BackendHandle,
                                       x: Int32, y: Int32,
                                       width: Int32, height: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    return Int32(backend.createWindow(x: Int(x), y: Int(y), width: Int(width), height: Int(height)))
}

@_cdecl("macos_backend_destroy_window")
public func macos_backend_destroy_window(_ handle: BackendHandle, windowId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.destroyWindow(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_map_window")
public func macos_backend_map_window(_ handle: BackendHandle, windowId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.mapWindow(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_unmap_window")
public func macos_backend_unmap_window(_ handle: BackendHandle, windowId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.unmapWindow(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_configure_window")
public func macos_backend_configure_window(_ handle: BackendHandle, windowId: Int32,
                                          x: Int32, y: Int32,
                                          width: Int32, height: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.configureWindow(id: Int(windowId), x: Int(x), y: Int(y),
                           width: Int(width), height: Int(height))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_raise_window")
public func macos_backend_raise_window(_ handle: BackendHandle, windowId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.raiseWindow(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_lower_window")
public func macos_backend_lower_window(_ handle: BackendHandle, windowId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.lowerWindow(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_set_window_title")
public func macos_backend_set_window_title(_ handle: BackendHandle, windowId: Int32,
                                          title: UnsafePointer<CChar>) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    let titleStr = String(cString: title)
    backend.setWindowTitle(id: Int(windowId), title: titleStr)
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_create_pixmap")
public func macos_backend_create_pixmap(_ handle: BackendHandle,
                                       width: Int32, height: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    return Int32(backend.createPixmap(width: Int(width), height: Int(height)))
}

@_cdecl("macos_backend_free_pixmap")
public func macos_backend_free_pixmap(_ handle: BackendHandle, pixmapId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.freePixmap(id: Int(pixmapId))
    return BackendResult.success.rawValue
}

// Drawing operations using CGContext

@_cdecl("macos_backend_clear_area")
public func macos_backend_clear_area(_ handle: BackendHandle, windowId: Int32,
                                    x: Int32, y: Int32, width: Int32, height: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    guard let context = backend.getWindowContext(id: Int(windowId)) else { return BackendResult.error.rawValue }

    let rect = CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(width), height: CGFloat(height))
    context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
    context.fill(rect)

    backend.releaseWindowContext(id: Int(windowId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_draw_rectangle")
public func macos_backend_draw_rectangle(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                        x: Int32, y: Int32, width: Int32, height: Int32,
                                        r: Float, g: Float, b: Float, lineWidth: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    let rect = CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(width), height: CGFloat(height))
    ctx.setStrokeColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.setLineWidth(CGFloat(lineWidth))
    ctx.stroke(rect)

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_fill_rectangle")
public func macos_backend_fill_rectangle(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                        x: Int32, y: Int32, width: Int32, height: Int32,
                                        r: Float, g: Float, b: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    let rect = CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(width), height: CGFloat(height))
    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.fill(rect)

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_draw_line")
public func macos_backend_draw_line(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                   x1: Int32, y1: Int32, x2: Int32, y2: Int32,
                                   r: Float, g: Float, b: Float, lineWidth: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    ctx.setStrokeColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.setLineWidth(CGFloat(lineWidth))
    ctx.move(to: CGPoint(x: CGFloat(x1), y: CGFloat(y1)))
    ctx.addLine(to: CGPoint(x: CGFloat(x2), y: CGFloat(y2)))
    ctx.strokePath()

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_flush")
public func macos_backend_flush(_ handle: BackendHandle) -> Int32 {
    DispatchQueue.main.sync {
        NSApplication.shared.updateWindows()
    }
    return BackendResult.success.rawValue
}
