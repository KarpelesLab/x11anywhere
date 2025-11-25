// MacOSBackend.swift - Swift wrapper for macOS Cocoa/Core Graphics operations
// Exposes C API for Rust FFI

import Cocoa
import Foundation
import ImageIO
import QuartzCore

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

// MARK: - Backing Buffer Storage

class X11BackingBuffer {
    var context: CGContext?
    var width: Int
    var height: Int

    init(width: Int, height: Int) {
        self.width = max(width, 1)
        self.height = max(height, 1)

        // Create a bitmap context for our backing store
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue

        if let ctx = CGContext(data: nil,
                               width: self.width,
                               height: self.height,
                               bitsPerComponent: 8,
                               bytesPerRow: self.width * 4,
                               space: colorSpace,
                               bitmapInfo: bitmapInfo) {
            // Fill with white background
            ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            ctx.fill(CGRect(x: 0, y: 0, width: self.width, height: self.height))
            self.context = ctx
        }
    }

    func makeNSImage() -> NSImage? {
        guard let cgImage = context?.makeImage() else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: width, height: height))
    }
}

// MARK: - Custom View that draws buffer content directly

class X11ContentView: NSView {
    var buffer: X11BackingBuffer?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    // Use flipped coordinates to match X11 (origin at top-left)
    override var isFlipped: Bool { return true }

    override func draw(_ dirtyRect: NSRect) {
        guard let cgImage = buffer?.context?.makeImage() else {
            NSLog("X11ContentView.draw: no CGImage available")
            NSColor.white.setFill()
            dirtyRect.fill()
            return
        }

        guard let context = NSGraphicsContext.current?.cgContext else {
            NSLog("X11ContentView.draw: no graphics context!")
            return
        }

        NSLog("X11ContentView.draw: drawing \(cgImage.width)x\(cgImage.height) in bounds \(bounds)")

        // Since isFlipped=true, view origin is top-left
        // But CGContext.draw expects bottom-left, so we need to flip
        context.saveGState()
        context.translateBy(x: 0, y: bounds.height)
        context.scaleBy(x: 1, y: -1)
        context.draw(cgImage, in: CGRect(x: 0, y: 0, width: bounds.width, height: bounds.height))
        context.restoreGState()
    }

    func updateContents() {
        NSLog("X11ContentView.updateContents: requesting display")
        needsDisplay = true
        // Force immediate display on main thread
        if Thread.isMainThread {
            display()
        } else {
            DispatchQueue.main.async {
                self.display()
            }
        }
    }
}

// MARK: - Backend Class

class MacOSBackendImpl {
    var windows: [Int: NSWindow] = [:]
    var windowContentViews: [Int: X11ContentView] = [:]
    var windowBuffers: [Int: X11BackingBuffer] = [:]
    var contexts: [Int: CGContext] = [:]
    var nextId: Int = 1
    var screenWidth: Int = 1920  // Default fallback
    var screenHeight: Int = 1080 // Default fallback
    var screenWidthMM: Int = 508
    var screenHeightMM: Int = 285

    init() {
        // Initialize NSApplication on main thread
        if Thread.isMainThread {
            initializeOnMainThread()
        } else {
            DispatchQueue.main.sync {
                self.initializeOnMainThread()
            }
        }
    }

    private func initializeOnMainThread() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)

        // Get screen dimensions
        if let screen = NSScreen.main {
            let frame = screen.frame
            screenWidth = Int(frame.width)
            screenHeight = Int(frame.height)

            // Estimate physical size (72 DPI base * backing scale)
            let backingScale = screen.backingScaleFactor
            let dpi = 72.0 * backingScale
            screenWidthMM = Int(Double(screenWidth) * 25.4 / dpi)
            screenHeightMM = Int(Double(screenHeight) * 25.4 / dpi)
        }
        // If NSScreen.main is nil, we keep the default values (1920x1080)
    }

    func createWindow(x: Int, y: Int, width: Int, height: Int) -> Int {
        var windowId = 0
        DispatchQueue.main.sync {
            let id = self.nextId
            self.nextId += 1

            // Convert Y coordinate (X11 top-left to macOS bottom-left)
            let flippedY = self.screenHeight - y - height

            NSLog("createWindow: id=\(id), x=\(x), y=\(y) (flipped=\(flippedY)), size=\(width)x\(height), screen=\(self.screenWidth)x\(self.screenHeight)")

            let rect = NSRect(x: CGFloat(x), y: CGFloat(flippedY),
                            width: CGFloat(width), height: CGFloat(height))

            let styleMask: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable]
            let window = NSWindow(contentRect: rect,
                                styleMask: styleMask,
                                backing: .buffered,
                                defer: false)

            window.title = "X11 Window"
            window.backgroundColor = .white

            // Create backing buffer and content view
            let buffer = X11BackingBuffer(width: width, height: height)
            let contentView = X11ContentView(frame: NSRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height)))
            contentView.buffer = buffer

            window.contentView = contentView

            self.windows[id] = window
            self.windowContentViews[id] = contentView
            self.windowBuffers[id] = buffer
            windowId = id

            NSLog("createWindow: created window \(id), buffer context: \(buffer.context != nil)")
        }
        return windowId
    }

    func destroyWindow(id: Int) {
        DispatchQueue.main.sync {
            self.windowContentViews.removeValue(forKey: id)
            self.windowBuffers.removeValue(forKey: id)
            if let window = self.windows.removeValue(forKey: id) {
                window.close()
            }
        }
    }

    func mapWindow(id: Int) {
        DispatchQueue.main.sync {
            NSLog("mapWindow: id=\(id)")
            if let window = self.windows[id] {
                // Make window appear on all spaces (including full screen apps)
                window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

                // Use a very high window level to ensure it appears on top
                window.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)))

                // Activate the app and force window to front
                NSApplication.shared.activate(ignoringOtherApps: true)

                // Show the window
                window.makeKeyAndOrderFront(nil)
                window.orderFrontRegardless()

                NSLog("mapWindow: window frame=\(window.frame), isVisible=\(window.isVisible)")

                // Update the content view with the current buffer content
                if let contentView = self.windowContentViews[id] {
                    contentView.updateContents()
                    NSLog("mapWindow: updated contentView, buffer context: \(contentView.buffer?.context != nil)")
                }
                window.display()

                // Pump the run loop to process window display
                for _ in 0..<10 {
                    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
                }
                NSLog("mapWindow: done, isVisible=\(window.isVisible)")
            } else {
                NSLog("mapWindow: ERROR - window \(id) not found!")
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
        // Return the backing context from our buffer
        return self.windowBuffers[id]?.context
    }

    func releaseWindowContext(id: Int) {
        // Save debug image BEFORE dispatching (this is called from background thread)
        if let buffer = self.windowBuffers[id], let context = buffer.context, let cgImage = context.makeImage() {
            let debugPath = "/tmp/x11anywhere_buffer_\(id).png"
            let url = URL(fileURLWithPath: debugPath)
            if let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) {
                CGImageDestinationAddImage(destination, cgImage, nil)
                CGImageDestinationFinalize(destination)
            }
        }

        // Update the content view asynchronously - don't block the request thread
        // Use async to avoid deadlock with CFRunLoopRun on main thread
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            if let contentView = self.windowContentViews[id] {
                contentView.updateContents()
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
    // Debug: write to a file to confirm this function is called
    let debugPath = "/tmp/x11anywhere_debug.log"
    let debugMsg = "macos_backend_create: called at \(Date())\n"
    if let data = debugMsg.data(using: .utf8) {
        if FileManager.default.fileExists(atPath: debugPath) {
            if let fileHandle = FileHandle(forWritingAtPath: debugPath) {
                fileHandle.seekToEndOfFile()
                fileHandle.write(data)
                fileHandle.closeFile()
            }
        } else {
            FileManager.default.createFile(atPath: debugPath, contents: data, attributes: nil)
        }
    }
    NSLog("macos_backend_create: called")

    let backend = MacOSBackendImpl()

    // Debug: log screen info
    let msg2 = "macos_backend_create: backend created, screen=\(backend.screenWidth)x\(backend.screenHeight)\n"
    if let data = msg2.data(using: .utf8) {
        if let fileHandle = FileHandle(forWritingAtPath: debugPath) {
            fileHandle.seekToEndOfFile()
            fileHandle.write(data)
            fileHandle.closeFile()
        }
    }
    NSLog("macos_backend_create: backend created, screen=\(backend.screenWidth)x\(backend.screenHeight)")

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
    // Debug logging
    let debugPath = "/tmp/x11anywhere_debug.log"
    let msg = "macos_backend_create_window: x=\(x), y=\(y), width=\(width), height=\(height)\n"
    if let data = msg.data(using: .utf8) {
        if let fileHandle = FileHandle(forWritingAtPath: debugPath) {
            fileHandle.seekToEndOfFile()
            fileHandle.write(data)
            fileHandle.closeFile()
        }
    }
    NSLog("macos_backend_create_window: x=\(x), y=\(y), width=\(width), height=\(height)")

    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    let windowId = Int32(backend.createWindow(x: Int(x), y: Int(y), width: Int(width), height: Int(height)))

    let msg2 = "macos_backend_create_window: created window id=\(windowId)\n"
    if let data = msg2.data(using: .utf8) {
        if let fileHandle = FileHandle(forWritingAtPath: debugPath) {
            fileHandle.seekToEndOfFile()
            fileHandle.write(data)
            fileHandle.closeFile()
        }
    }

    return windowId
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

    // Debug: write to a file to confirm this function is called
    let debugMsg = "fill_rectangle: isWindow=\(isWindow), drawable=\(drawableId), rect=(\(x),\(y),\(width),\(height)), color=(\(r),\(g),\(b))\n"
    let debugPath = "/tmp/x11anywhere_debug.log"
    if let data = debugMsg.data(using: .utf8) {
        if FileManager.default.fileExists(atPath: debugPath) {
            if let fileHandle = FileHandle(forWritingAtPath: debugPath) {
                fileHandle.seekToEndOfFile()
                fileHandle.write(data)
                fileHandle.closeFile()
            }
        } else {
            FileManager.default.createFile(atPath: debugPath, contents: data, attributes: nil)
        }
    }

    NSLog("fill_rectangle: isWindow=\(isWindow), drawable=\(drawableId), rect=(\(x),\(y),\(width),\(height)), color=(\(r),\(g),\(b))")

    let context: CGContext?
    let bufferHeight: Int
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
        bufferHeight = backend.windowBuffers[Int(drawableId)]?.height ?? 600
        NSLog("fill_rectangle: got window context: \(context != nil), bufferHeight: \(bufferHeight)")
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
        // For pixmaps, try to get height from context or use a default
        bufferHeight = context?.height ?? 600
    }

    guard let ctx = context else {
        NSLog("fill_rectangle: ERROR - no context!")
        return BackendResult.error.rawValue
    }

    // Convert X11 coordinates (origin at top-left) to CGContext (origin at bottom-left)
    let flippedY = CGFloat(bufferHeight) - CGFloat(y) - CGFloat(height)
    let rect = CGRect(x: CGFloat(x), y: flippedY, width: CGFloat(width), height: CGFloat(height))
    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.fill(rect)
    NSLog("fill_rectangle: filled rect at flipped y=\(flippedY) (original y=\(y))")

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

@_cdecl("macos_backend_draw_arc")
public func macos_backend_draw_arc(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                   x: Int32, y: Int32, width: Int32, height: Int32,
                                   angle1: Int32, angle2: Int32,
                                   r: Float, g: Float, b: Float, lineWidth: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    // Convert X11 angles (1/64 degrees, 0 at 3 o'clock counterclockwise) to radians
    let startAngle = CGFloat(angle1) * CGFloat.pi / (180.0 * 64.0)
    let endAngle = CGFloat(angle1 + angle2) * CGFloat.pi / (180.0 * 64.0)

    // Calculate ellipse center and radii
    let centerX = CGFloat(x) + CGFloat(width) / 2.0
    let centerY = CGFloat(y) + CGFloat(height) / 2.0
    let radiusX = CGFloat(width) / 2.0
    let radiusY = CGFloat(height) / 2.0

    ctx.setStrokeColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.setLineWidth(CGFloat(lineWidth))

    // Save context state
    ctx.saveGState()

    // Transform to draw ellipse as circle, then scale
    ctx.translateBy(x: centerX, y: centerY)
    ctx.scaleBy(x: radiusX, y: radiusY)

    // Draw arc (Core Graphics uses clockwise from 3 o'clock, opposite of X11)
    ctx.addArc(center: CGPoint.zero, radius: 1.0, startAngle: -startAngle, endAngle: -endAngle, clockwise: true)
    ctx.strokePath()

    ctx.restoreGState()

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_fill_arc")
public func macos_backend_fill_arc(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                   x: Int32, y: Int32, width: Int32, height: Int32,
                                   angle1: Int32, angle2: Int32,
                                   r: Float, g: Float, b: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    let startAngle = CGFloat(angle1) * CGFloat.pi / (180.0 * 64.0)
    let endAngle = CGFloat(angle1 + angle2) * CGFloat.pi / (180.0 * 64.0)

    let centerX = CGFloat(x) + CGFloat(width) / 2.0
    let centerY = CGFloat(y) + CGFloat(height) / 2.0
    let radiusX = CGFloat(width) / 2.0
    let radiusY = CGFloat(height) / 2.0

    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))

    ctx.saveGState()
    ctx.translateBy(x: centerX, y: centerY)
    ctx.scaleBy(x: radiusX, y: radiusY)

    // Draw pie slice
    ctx.move(to: CGPoint.zero)
    ctx.addArc(center: CGPoint.zero, radius: 1.0, startAngle: -startAngle, endAngle: -endAngle, clockwise: true)
    ctx.closePath()
    ctx.fillPath()

    ctx.restoreGState()

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_fill_polygon")
public func macos_backend_fill_polygon(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                       points: UnsafePointer<Int32>, pointCount: Int32,
                                       r: Float, g: Float, b: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))

    if pointCount > 0 {
        // First point - move to
        let x0 = CGFloat(points[0])
        let y0 = CGFloat(points[1])
        ctx.move(to: CGPoint(x: x0, y: y0))

        // Remaining points - add lines
        for i in 1..<Int(pointCount) {
            let x = CGFloat(points[i * 2])
            let y = CGFloat(points[i * 2 + 1])
            ctx.addLine(to: CGPoint(x: x, y: y))
        }

        ctx.closePath()
        ctx.fillPath()
    }

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_put_image")
public func macos_backend_put_image(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                    width: Int32, height: Int32, dst_x: Int32, dst_y: Int32,
                                    depth: Int32, format: Int32, data: UnsafePointer<UInt8>, dataLength: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    // X11 image formats:
    // 0 = Bitmap, 1 = XYPixmap, 2 = ZPixmap (packed pixels)
    if format == 2 {
        // ZPixmap - packed pixel format
        // Determine bytes per pixel from depth
        let bytesPerPixel: Int
        if depth <= 8 {
            bytesPerPixel = 1
        } else if depth <= 16 {
            bytesPerPixel = 2
        } else if depth <= 24 {
            bytesPerPixel = 3
        } else {
            bytesPerPixel = 4
        }

        let bytesPerRow = Int(width) * bytesPerPixel

        // Create a data provider from the raw image data
        guard let dataProvider = CGDataProvider(dataInfo: nil,
                                                 data: data,
                                                 size: Int(dataLength),
                                                 releaseData: { _, _, _ in }) else {
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.error.rawValue
        }

        // Create CGImage from the data
        // Assuming BGRA format for 32bpp, RGB for 24bpp, etc.
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo: CGBitmapInfo

        if bytesPerPixel == 4 {
            bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        } else if bytesPerPixel == 3 {
            bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.none.rawValue)
        } else {
            // Unsupported depth for now
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.error.rawValue
        }

        guard let cgImage = CGImage(width: Int(width),
                                    height: Int(height),
                                    bitsPerComponent: 8,
                                    bitsPerPixel: bytesPerPixel * 8,
                                    bytesPerRow: bytesPerRow,
                                    space: colorSpace,
                                    bitmapInfo: bitmapInfo,
                                    provider: dataProvider,
                                    decode: nil,
                                    shouldInterpolate: false,
                                    intent: .defaultIntent) else {
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.error.rawValue
        }

        // Draw the image
        let rect = CGRect(x: CGFloat(dst_x), y: CGFloat(dst_y), width: CGFloat(width), height: CGFloat(height))
        ctx.draw(cgImage, in: rect)
    }

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_get_image")
public func macos_backend_get_image(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                    x: Int32, y: Int32, width: Int32, height: Int32,
                                    buffer: UnsafeMutablePointer<UInt8>, bufferSize: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else { return BackendResult.error.rawValue }

    // Create a CGImage from the context
    guard let cgImage = ctx.makeImage() else {
        if isWindow != 0 {
            backend.releaseWindowContext(id: Int(drawableId))
        }
        return BackendResult.error.rawValue
    }

    // Crop to the requested region
    let cropRect = CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(width), height: CGFloat(height))
    guard let croppedImage = cgImage.cropping(to: cropRect) else {
        if isWindow != 0 {
            backend.releaseWindowContext(id: Int(drawableId))
        }
        return BackendResult.error.rawValue
    }

    // Create a bitmap context to read the pixels
    let bytesPerPixel = 4 // RGBA
    let bytesPerRow = Int(width) * bytesPerPixel
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let bitmapInfo = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)

    guard let bitmapContext = CGContext(data: buffer,
                                        width: Int(width),
                                        height: Int(height),
                                        bitsPerComponent: 8,
                                        bytesPerRow: bytesPerRow,
                                        space: colorSpace,
                                        bitmapInfo: bitmapInfo.rawValue) else {
        if isWindow != 0 {
            backend.releaseWindowContext(id: Int(drawableId))
        }
        return BackendResult.error.rawValue
    }

    // Draw the cropped image into the bitmap context (this copies the pixels to our buffer)
    bitmapContext.draw(croppedImage, in: CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height)))

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
    }
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_copy_area")
public func macos_backend_copy_area(_ handle: BackendHandle,
                                    srcIsWindow: Int32, srcDrawableId: Int32,
                                    dstIsWindow: Int32, dstDrawableId: Int32,
                                    srcX: Int32, srcY: Int32,
                                    width: Int32, height: Int32,
                                    dstX: Int32, dstY: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    // Get source context
    let srcContext: CGContext?
    if srcIsWindow != 0 {
        srcContext = backend.getWindowContext(id: Int(srcDrawableId))
    } else {
        srcContext = backend.getPixmapContext(id: Int(srcDrawableId))
    }

    guard let srcCtx = srcContext else {
        return BackendResult.error.rawValue
    }

    // Create a CGImage from the source context
    guard let fullImage = srcCtx.makeImage() else {
        if srcIsWindow != 0 {
            backend.releaseWindowContext(id: Int(srcDrawableId))
        }
        return BackendResult.error.rawValue
    }

    // Crop to the source rectangle
    let cropRect = CGRect(x: CGFloat(srcX), y: CGFloat(srcY),
                          width: CGFloat(width), height: CGFloat(height))
    guard let croppedImage = fullImage.cropping(to: cropRect) else {
        if srcIsWindow != 0 {
            backend.releaseWindowContext(id: Int(srcDrawableId))
        }
        return BackendResult.error.rawValue
    }

    // Release source context
    if srcIsWindow != 0 {
        backend.releaseWindowContext(id: Int(srcDrawableId))
    }

    // Get destination context
    let dstContext: CGContext?
    if dstIsWindow != 0 {
        dstContext = backend.getWindowContext(id: Int(dstDrawableId))
    } else {
        dstContext = backend.getPixmapContext(id: Int(dstDrawableId))
    }

    guard let dstCtx = dstContext else {
        return BackendResult.error.rawValue
    }

    // Draw the cropped image to the destination
    let dstRect = CGRect(x: CGFloat(dstX), y: CGFloat(dstY),
                         width: CGFloat(width), height: CGFloat(height))
    dstCtx.draw(croppedImage, in: dstRect)

    // Release destination context
    if dstIsWindow != 0 {
        backend.releaseWindowContext(id: Int(dstDrawableId))
    }

    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_flush")
public func macos_backend_flush(_ handle: BackendHandle) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    DispatchQueue.main.sync {
        NSLog("flush: updating \(backend.windowContentViews.count) windows")

        // Update all content views
        for (id, contentView) in backend.windowContentViews {
            if let buffer = backend.windowBuffers[id], let window = backend.windows[id] {
                guard let cgImage = buffer.context?.makeImage() else {
                    NSLog("flush: window \(id) - no CGImage available!")
                    continue
                }

                NSLog("flush: window \(id) - cgImage: \(cgImage.width)x\(cgImage.height), contentView frame: \(contentView.frame)")

                // Update the layer contents
                contentView.updateContents()
                window.display()

                // Debug: save buffer to file
                let debugPath = "/tmp/x11anywhere_debug_window_\(id).png"
                let url = URL(fileURLWithPath: debugPath)
                if let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) {
                    CGImageDestinationAddImage(destination, cgImage, nil)
                    if CGImageDestinationFinalize(destination) {
                        NSLog("flush: saved debug image to \(debugPath)")
                    }
                }
            }
        }
        NSApplication.shared.updateWindows()

        // Pump the run loop to process display updates
        for _ in 0..<5 {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        }
    }
    return BackendResult.success.rawValue
}

/// Save a window's backing buffer to a PNG file (for debugging/testing)
@_cdecl("macos_backend_save_window_to_png")
public func macos_backend_save_window_to_png(_ handle: BackendHandle, windowId: Int32, path: UnsafePointer<CChar>) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    let pathStr = String(cString: path)

    guard let buffer = backend.windowBuffers[Int(windowId)],
          let context = buffer.context,
          let cgImage = context.makeImage() else {
        return BackendResult.error.rawValue
    }

    let url = URL(fileURLWithPath: pathStr)
    guard let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) else {
        return BackendResult.error.rawValue
    }

    CGImageDestinationAddImage(destination, cgImage, nil)
    if CGImageDestinationFinalize(destination) {
        return BackendResult.success.rawValue
    }
    return BackendResult.error.rawValue
}

@_cdecl("macos_backend_poll_event")
public func macos_backend_poll_event(
    _ handle: BackendHandle,
    eventType: UnsafeMutablePointer<Int32>,
    windowId: UnsafeMutablePointer<Int32>,
    x: UnsafeMutablePointer<Int32>,
    y: UnsafeMutablePointer<Int32>,
    width: UnsafeMutablePointer<Int32>,
    height: UnsafeMutablePointer<Int32>,
    keycode: UnsafeMutablePointer<Int32>,
    button: UnsafeMutablePointer<Int32>,
    state: UnsafeMutablePointer<Int32>,
    time: UnsafeMutablePointer<Int32>
) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    var hasEvent = false
    var evtType: Int32 = 0
    var evtWindowId: Int32 = 0
    var evtX: Int32 = 0
    var evtY: Int32 = 0
    var evtWidth: Int32 = 0
    var evtHeight: Int32 = 0
    var evtKeycode: Int32 = 0
    var evtButton: Int32 = 0
    var evtState: Int32 = 0
    var evtTime: Int32 = 0

    DispatchQueue.main.sync {
        if let nsEvent = NSApplication.shared.nextEvent(matching: .any, until: nil, inMode: .default, dequeue: true) {
            hasEvent = true

            // Find which window this event is for
            if let eventWindow = nsEvent.window {
                for (id, window) in backend.windows {
                    if window === eventWindow {
                        evtWindowId = Int32(id)
                        break
                    }
                }
            }

            evtTime = Int32(nsEvent.timestamp * 1000) // Convert to milliseconds

            switch nsEvent.type {
            case .leftMouseDown:
                evtType = 5 // buttonpress
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .rightMouseDown:
                evtType = 5 // buttonpress
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .otherMouseDown:
                evtType = 5 // buttonpress
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .leftMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .rightMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .otherMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
                evtType = 7 // motion
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .keyDown:
                evtType = 3 // keypress
                evtKeycode = Int32(nsEvent.keyCode)

            case .keyUp:
                evtType = 4 // keyrelease
                evtKeycode = Int32(nsEvent.keyCode)

            default:
                hasEvent = false
            }

            // Dispatch the event to the application
            NSApplication.shared.sendEvent(nsEvent)
        }
    }

    if hasEvent {
        eventType.pointee = evtType
        windowId.pointee = evtWindowId
        x.pointee = evtX
        y.pointee = evtY
        width.pointee = evtWidth
        height.pointee = evtHeight
        keycode.pointee = evtKeycode
        button.pointee = evtButton
        state.pointee = evtState
        time.pointee = evtTime
        return 1 // Has event
    }
    return 0 // No event
}

@_cdecl("macos_backend_wait_for_event")
public func macos_backend_wait_for_event(
    _ handle: BackendHandle,
    eventType: UnsafeMutablePointer<Int32>,
    windowId: UnsafeMutablePointer<Int32>,
    x: UnsafeMutablePointer<Int32>,
    y: UnsafeMutablePointer<Int32>,
    width: UnsafeMutablePointer<Int32>,
    height: UnsafeMutablePointer<Int32>,
    keycode: UnsafeMutablePointer<Int32>,
    button: UnsafeMutablePointer<Int32>,
    state: UnsafeMutablePointer<Int32>,
    time: UnsafeMutablePointer<Int32>
) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    var evtType: Int32 = 0
    var evtWindowId: Int32 = 0
    var evtX: Int32 = 0
    var evtY: Int32 = 0
    var evtWidth: Int32 = 0
    var evtHeight: Int32 = 0
    var evtKeycode: Int32 = 0
    var evtButton: Int32 = 0
    var evtState: Int32 = 0
    var evtTime: Int32 = 0

    DispatchQueue.main.sync {
        // Wait indefinitely for an event
        if let nsEvent = NSApplication.shared.nextEvent(matching: .any, until: .distantFuture, inMode: .default, dequeue: true) {
            // Find which window this event is for
            if let eventWindow = nsEvent.window {
                for (id, window) in backend.windows {
                    if window === eventWindow {
                        evtWindowId = Int32(id)
                        break
                    }
                }
            }

            evtTime = Int32(nsEvent.timestamp * 1000) // Convert to milliseconds

            switch nsEvent.type {
            case .leftMouseDown:
                evtType = 5 // buttonpress
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .rightMouseDown:
                evtType = 5 // buttonpress
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .otherMouseDown:
                evtType = 5 // buttonpress
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .leftMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .rightMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .otherMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
                evtType = 7 // motion
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = Int32(nsEvent.locationInWindow.y)

            case .keyDown:
                evtType = 3 // keypress
                evtKeycode = Int32(nsEvent.keyCode)

            case .keyUp:
                evtType = 4 // keyrelease
                evtKeycode = Int32(nsEvent.keyCode)

            default:
                break
            }

            // Dispatch the event to the application
            NSApplication.shared.sendEvent(nsEvent)
        }
    }

    eventType.pointee = evtType
    windowId.pointee = evtWindowId
    x.pointee = evtX
    y.pointee = evtY
    width.pointee = evtWidth
    height.pointee = evtHeight
    keycode.pointee = evtKeycode
    button.pointee = evtButton
    state.pointee = evtState
    time.pointee = evtTime
    return 1 // Has event
}
