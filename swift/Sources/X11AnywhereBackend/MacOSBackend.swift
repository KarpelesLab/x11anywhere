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
        // Use premultipliedLast (RGBA) for better compatibility with CALayer display
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue

        if let ctx = CGContext(data: nil,
                               width: self.width,
                               height: self.height,
                               bitsPerComponent: 8,
                               bytesPerRow: self.width * 4,
                               space: colorSpace,
                               bitmapInfo: bitmapInfo) {
            // No CTM flip here - X11 draws with Y=0 at top, which means row 0 is at top
            // The flip will happen in the draw() method when displaying

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
    private var trackingArea: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupTrackingArea()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupTrackingArea()
    }

    private func setupTrackingArea() {
        let options: NSTrackingArea.Options = [
            .mouseEnteredAndExited,
            .mouseMoved,
            .activeAlways,
            .inVisibleRect
        ]
        trackingArea = NSTrackingArea(rect: bounds, options: options, owner: self, userInfo: nil)
        addTrackingArea(trackingArea!)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        // Remove old tracking area and add a new one when the view resizes
        if let area = trackingArea {
            removeTrackingArea(area)
        }
        setupTrackingArea()
    }

    override var isFlipped: Bool { return true }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        guard let buffer = self.buffer, let ctx = buffer.context, let cgImage = ctx.makeImage() else {
            return
        }

        guard let currentCtx = NSGraphicsContext.current?.cgContext else {
            return
        }

        let imageWidth = CGFloat(cgImage.width)
        let imageHeight = CGFloat(cgImage.height)

        // Draw at y=30 to account for window title bar offset
        currentCtx.draw(cgImage, in: CGRect(x: 0, y: 30, width: imageWidth, height: imageHeight))
    }

    func updateContents() {
        guard let buffer = self.buffer, let ctx = buffer.context, let cgImage = ctx.makeImage() else {
            NSLog("X11ContentView.updateContents: no CGImage available, buffer=\(self.buffer != nil)")
            return
        }

        NSLog("X11ContentView.updateContents: setting image \(cgImage.width)x\(cgImage.height), bpc=\(cgImage.bitsPerComponent), bpp=\(cgImage.bitsPerPixel)")

        // Mark view as needing redraw and force immediate display
        self.setNeedsDisplay(self.bounds)
        self.displayIfNeeded()
    }
}

// MARK: - Backend Class

class MacOSBackendImpl {
    var windows: [Int: NSWindow] = [:]
    var windowContentViews: [Int: X11ContentView] = [:]
    var windowBuffers: [Int: X11BackingBuffer] = [:]
    var contexts: [Int: CGContext] = [:]
    var pixmapSizes: [Int: (Int, Int)] = [:]  // Pixmap dimensions for coordinate conversion
    var cursors: [Int: NSCursor] = [:]
    var windowCursors: [Int: NSCursor] = [:]  // Per-window cursor
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
        NSLog("destroyWindow: id=\(id)")
        DispatchQueue.main.sync {
            self.windowContentViews.removeValue(forKey: id)
            self.windowBuffers.removeValue(forKey: id)
            if let window = self.windows.removeValue(forKey: id) {
                NSLog("destroyWindow: closing window \(id)")
                // Use orderOut instead of close to prevent app termination
                // when last window closes
                window.orderOut(nil)
                NSLog("destroyWindow: window closed")
            } else {
                NSLog("destroyWindow: window \(id) not found in windows dict")
            }
        }
    }

    func mapWindow(id: Int) {
        DispatchQueue.main.sync {
            NSLog("mapWindow: id=\(id)")
            if let window = self.windows[id] {
                // Normal window level - can go behind other windows
                window.level = .normal

                // Activate the app and show the window
                NSApplication.shared.activate(ignoringOtherApps: true)

                // Show the window
                window.makeKeyAndOrderFront(nil)

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

                // Resize the backing buffer if size changed
                if let oldBuffer = self.windowBuffers[id],
                   (oldBuffer.width != width || oldBuffer.height != height) {
                    NSLog("configureWindow: resizing buffer from \(oldBuffer.width)x\(oldBuffer.height) to \(width)x\(height)")

                    // Create new buffer with new size (starts with white background)
                    // Don't copy old content - X11 semantics require client to redraw
                    // after receiving ConfigureNotify/Expose events
                    let newBuffer = X11BackingBuffer(width: width, height: height)
                    self.windowBuffers[id] = newBuffer

                    // Update content view with new buffer
                    if let contentView = self.windowContentViews[id] {
                        contentView.buffer = newBuffer
                        contentView.frame = NSRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height))
                        contentView.updateContents()
                    }
                }
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
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue

        if let context = CGContext(data: nil,
                                   width: width,
                                   height: height,
                                   bitsPerComponent: 8,
                                   bytesPerRow: width * 4,
                                   space: colorSpace,
                                   bitmapInfo: bitmapInfo) {
            // Apply the same coordinate transformation as windows:
            // Flip Y-axis so X11 coordinates (Y=0 at top) work correctly
            context.translateBy(x: 0, y: CGFloat(height))
            context.scaleBy(x: 1.0, y: -1.0)

            // Fill with white background (same as windows)
            context.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))

            self.contexts[id] = context
            self.pixmapSizes[id] = (width, height)
            return id
        }
        return 0
    }

    func getPixmapContext(id: Int) -> CGContext? {
        return self.contexts[id]
    }

    func freePixmap(id: Int) {
        self.contexts.removeValue(forKey: id)
        self.pixmapSizes.removeValue(forKey: id)
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
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
        NSLog("fill_rectangle: got window context: \(context != nil)")
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else {
        NSLog("fill_rectangle: ERROR - no context!")
        return BackendResult.error.rawValue
    }

    // Use X11 coordinates directly - CTM transform in X11BackingBuffer handles coordinate flipping
    let rect = CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(width), height: CGFloat(height))
    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))
    ctx.fill(rect)
    NSLog("fill_rectangle: filled rect at y=\(y)")

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

    // Save context state
    ctx.saveGState()

    // Transform to draw ellipse as circle, then scale
    ctx.translateBy(x: centerX, y: centerY)
    ctx.scaleBy(x: radiusX, y: radiusY)

    // Set line width compensated for the scale transformation
    // Use the smaller radius to ensure consistent stroke width
    let scaleFactor = min(radiusX, radiusY)
    if scaleFactor > 0 {
        ctx.setLineWidth(CGFloat(lineWidth) / scaleFactor)
    } else {
        ctx.setLineWidth(CGFloat(lineWidth))
    }

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

    NSLog("fill_polygon: isWindow=\(isWindow), drawableId=\(drawableId), pointCount=\(pointCount), color=(\(r),\(g),\(b))")

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else {
        NSLog("fill_polygon: ERROR - context not found")
        return BackendResult.error.rawValue
    }

    ctx.setFillColor(CGColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1))

    if pointCount > 0 {
        // First point - move to
        let x0 = CGFloat(points[0])
        let y0 = CGFloat(points[1])

        // Log the polygon points
        var pointsStr = "(\(Int(x0)),\(Int(y0)))"
        for i in 1..<Int(pointCount) {
            let x = CGFloat(points[i * 2])
            let y = CGFloat(points[i * 2 + 1])
            pointsStr += " (\(Int(x)),\(Int(y)))"
        }
        NSLog("fill_polygon: drawing path: \(pointsStr)")

        ctx.beginPath()
        ctx.move(to: CGPoint(x: x0, y: y0))

        // Remaining points - add lines
        for i in 1..<Int(pointCount) {
            let x = CGFloat(points[i * 2])
            let y = CGFloat(points[i * 2 + 1])
            ctx.addLine(to: CGPoint(x: x, y: y))
        }

        ctx.closePath()
        ctx.fillPath()

        // Check if something was drawn - scan for any non-white pixels
        if let img = ctx.makeImage(), let dp = img.dataProvider, let data = dp.data {
            let ptr = CFDataGetBytePtr(data)
            let length = CFDataGetLength(data)
            var nonWhiteCount = 0
            var firstNonWhite: (Int, Int, UInt8, UInt8, UInt8)? = nil
            // Scan all pixels for non-white
            var i = 0
            while i < length - 3 {
                let pr = ptr![i]
                let pg = ptr![i + 1]
                let pb = ptr![i + 2]
                if pr != 255 || pg != 255 || pb != 255 {
                    nonWhiteCount += 1
                    if firstNonWhite == nil {
                        let pixelIdx = i / 4
                        let px = pixelIdx % ctx.width
                        let py = pixelIdx / ctx.width
                        firstNonWhite = (px, py, pr, pg, pb)
                    }
                }
                i += 4
            }
            if let (px, py, pr, pg, pb) = firstNonWhite {
                NSLog("fill_polygon: drawableId=\(drawableId) has \(nonWhiteCount) non-white pixels, first at (\(px),\(py)) RGB(\(pr),\(pg),\(pb))")
            } else {
                NSLog("fill_polygon: drawableId=\(drawableId) has 0 non-white pixels after fill!")
            }

            // Save debug PNG for the first few fill operations on this drawable
            let debugPath = "/tmp/x11anywhere_pixmap_\(drawableId)_fill.png"
            let url = URL(fileURLWithPath: debugPath)
            if let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) {
                CGImageDestinationAddImage(destination, img, nil)
                CGImageDestinationFinalize(destination)
                NSLog("fill_polygon: saved debug image to \(debugPath)")
            }
        }
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

    NSLog("put_image: isWindow=\(isWindow), drawableId=\(drawableId), size=\(width)x\(height), depth=\(depth), format=\(format), dataLen=\(dataLength)")

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
        NSLog("put_image: getWindowContext(\(drawableId)) = \(context != nil ? "found" : "nil")")
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
        NSLog("put_image: getPixmapContext(\(drawableId)) = \(context != nil ? "found" : "nil")")
    }

    guard let ctx = context else {
        NSLog("put_image: FAILED - context not found for drawable \(drawableId)")
        return BackendResult.error.rawValue
    }

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

        NSLog("put_image: ZPixmap depth=\(depth), bytesPerPixel=\(bytesPerPixel)")

        let bytesPerRow = Int(width) * bytesPerPixel

        // Create a data provider from the raw image data
        guard let dataProvider = CGDataProvider(dataInfo: nil,
                                                 data: data,
                                                 size: Int(dataLength),
                                                 releaseData: { _, _, _ in }) else {
            NSLog("put_image: FAILED - CGDataProvider creation failed")
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
        } else if bytesPerPixel == 2 {
            // 16-bit: RGB565 - use grayscale as approximation
            // CoreGraphics doesn't natively support RGB565, so we'll skip drawing for now
            // but return success to not break the app
            NSLog("put_image: 16-bit depth not fully supported, skipping draw")
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.success.rawValue
        } else {
            // 8-bit indexed color - skip for now but don't fail
            NSLog("put_image: 8-bit indexed color not fully supported, skipping draw")
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.success.rawValue
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
            NSLog("put_image: FAILED - CGImage creation failed")
            if isWindow != 0 {
                backend.releaseWindowContext(id: Int(drawableId))
            }
            return BackendResult.error.rawValue
        }

        // Draw the image
        // The context has a Y-flip CTM, but the incoming X11 image data has Y=0 at top.
        // When drawn normally, the CTM would flip the image upside down.
        // We counteract this by applying a local flip during drawing.
        ctx.saveGState()
        ctx.translateBy(x: CGFloat(dst_x), y: CGFloat(dst_y) + CGFloat(height))
        ctx.scaleBy(x: 1.0, y: -1.0)
        let drawRect = CGRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height))
        ctx.draw(cgImage, in: drawRect)
        ctx.restoreGState()
        NSLog("put_image: drew image at \(dst_x),\(dst_y)")
    } else if format == 0 {
        // Bitmap format (1-bit) - often used for shape masks
        // Skip but don't fail - we don't support shaped windows yet
        NSLog("put_image: Bitmap format (1-bit), skipping")
    } else {
        // XYPixmap format - rarely used, skip
        NSLog("put_image: XYPixmap format, skipping")
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
    // The cropped image is from a Y-flipped context (Y=0 at top = X11 orientation)
    // The bitmap context has default CGContext orientation (Y=0 at bottom)
    // We need to flip during draw to preserve X11 orientation in the output buffer
    bitmapContext.translateBy(x: 0, y: CGFloat(height))
    bitmapContext.scaleBy(x: 1.0, y: -1.0)
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

    NSLog("copy_area: src(isWindow=\(srcIsWindow), id=\(srcDrawableId)) -> dst(isWindow=\(dstIsWindow), id=\(dstDrawableId)), srcRect=(\(srcX),\(srcY),\(width),\(height)), dstPos=(\(dstX),\(dstY))")

    // Get source context and its dimensions
    let srcContext: CGContext?
    var srcHeight: Int = 0
    if srcIsWindow != 0 {
        srcContext = backend.getWindowContext(id: Int(srcDrawableId))
        srcHeight = backend.windowBuffers[Int(srcDrawableId)]?.height ?? 0
    } else {
        srcContext = backend.getPixmapContext(id: Int(srcDrawableId))
        srcHeight = backend.pixmapSizes[Int(srcDrawableId)]?.1 ?? 0
    }

    guard let srcCtx = srcContext else {
        NSLog("copy_area: ERROR - source context not found")
        return BackendResult.error.rawValue
    }

    // Create a CGImage from the source context
    // The CGImage is in raw bitmap coordinates (Y=0 at top-left of the bitmap)
    // Since we drew with Y-flip CTM, the bitmap stores content with Y=0 at what X11 considers the top
    guard let fullImage = srcCtx.makeImage() else {
        NSLog("copy_area: ERROR - makeImage failed")
        if srcIsWindow != 0 {
            backend.releaseWindowContext(id: Int(srcDrawableId))
        }
        return BackendResult.error.rawValue
    }

    NSLog("copy_area: fullImage size = \(fullImage.width)x\(fullImage.height), srcHeight=\(srcHeight)")

    // Debug: Check for non-white pixels in the source image - full scan
    if let dataProvider = fullImage.dataProvider, let data = dataProvider.data {
        let ptr = CFDataGetBytePtr(data)
        let length = CFDataGetLength(data)
        var nonWhiteCount = 0
        var firstNonWhite: (Int, Int, UInt8, UInt8, UInt8)? = nil
        // Full scan of all pixels
        var i = 0
        while i < length - 3 {
            let r = ptr![i]
            let g = ptr![i+1]
            let b = ptr![i+2]
            if r != 255 || g != 255 || b != 255 {
                nonWhiteCount += 1
                if firstNonWhite == nil {
                    let pixelIdx = i / 4
                    let px = pixelIdx % fullImage.width
                    let py = pixelIdx / fullImage.width
                    firstNonWhite = (px, py, r, g, b)
                }
            }
            i += 4
        }
        if let (px, py, r, g, b) = firstNonWhite {
            NSLog("copy_area: srcDrawableId=\(srcDrawableId) has \(nonWhiteCount) non-white pixels, first at (\(px),\(py)) RGB(\(r),\(g),\(b))")
        } else {
            NSLog("copy_area: srcDrawableId=\(srcDrawableId) has 0 non-white pixels!")
            // Save debug PNG to see what the pixmap looks like
            let debugPath = "/tmp/x11anywhere_pixmap_\(srcDrawableId)_copy.png"
            let url = URL(fileURLWithPath: debugPath)
            if let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) {
                CGImageDestinationAddImage(destination, fullImage, nil)
                CGImageDestinationFinalize(destination)
                NSLog("copy_area: saved debug image to \(debugPath)")
            }
        }
    }

    // Crop to the source rectangle
    // CGImage.cropping uses pixel coordinates where Y=0 is at the TOP of the image
    // (matching how the content was stored after Y-flip drawing)
    let cropRect = CGRect(x: CGFloat(srcX), y: CGFloat(srcY),
                          width: CGFloat(width), height: CGFloat(height))
    guard let croppedImage = fullImage.cropping(to: cropRect) else {
        NSLog("copy_area: ERROR - cropping failed for rect \(cropRect)")
        if srcIsWindow != 0 {
            backend.releaseWindowContext(id: Int(srcDrawableId))
        }
        return BackendResult.error.rawValue
    }

    NSLog("copy_area: cropped to \(croppedImage.width)x\(croppedImage.height)")

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
        NSLog("copy_area: ERROR - destination context not found")
        return BackendResult.error.rawValue
    }

    // Draw the cropped image to the destination
    // Both source and destination contexts have Y-flip CTMs applied.
    // The CGImage from makeImage() contains raw bitmap data where Y=0 is at the top.
    //
    // Problem: CGContext.draw() applies the context's CTM to the image drawing,
    // which causes the image to be vertically flipped when drawn to a Y-flipped context.
    //
    // Solution: Temporarily reset the CTM to identity for image drawing, then restore.
    // We draw directly to device coordinates, which bypasses the Y-flip issue.

    dstCtx.saveGState()

    // Reset to identity transform by applying the inverse of the current CTM
    // This allows us to draw directly in bitmap coordinates
    let ctm = dstCtx.ctm
    dstCtx.concatenate(ctm.inverted())

    // Log the CTM for debugging
    NSLog("copy_area: CTM before reset: a=\(ctm.a), b=\(ctm.b), c=\(ctm.c), d=\(ctm.d), tx=\(ctm.tx), ty=\(ctm.ty)")

    // Now we're in device coordinates where Y=0 is at the TOP of the bitmap
    // The CGImage also has Y=0 at top, so we can draw directly
    let drawRect = CGRect(x: CGFloat(dstX), y: CGFloat(dstY), width: CGFloat(width), height: CGFloat(height))
    dstCtx.draw(croppedImage, in: drawRect)

    dstCtx.restoreGState()

    NSLog("copy_area: drew image at (\(dstX),\(dstY))")

    // Debug: Check destination buffer after draw - show first non-white pixel position
    if let dstImg = dstCtx.makeImage(), let dp = dstImg.dataProvider, let data = dp.data {
        let ptr = CFDataGetBytePtr(data)
        let length = CFDataGetLength(data)
        var nonWhiteCount = 0
        var firstNonWhite: (Int, Int, UInt8, UInt8, UInt8)? = nil
        var i = 0
        while i < length - 3 {
            let r = ptr![i]
            let g = ptr![i+1]
            let b = ptr![i+2]
            if r != 255 || g != 255 || b != 255 {
                nonWhiteCount += 1
                if firstNonWhite == nil {
                    let pixelIdx = i / 4
                    let px = pixelIdx % dstImg.width
                    let py = pixelIdx / dstImg.width
                    firstNonWhite = (px, py, r, g, b)
                }
            }
            i += 4
        }
        if let (px, py, r, g, b) = firstNonWhite {
            NSLog("copy_area: destination dstId=\(dstDrawableId) after draw has \(nonWhiteCount) non-white pixels, first at (\(px),\(py)) RGB(\(r),\(g),\(b))")
        } else {
            NSLog("copy_area: destination dstId=\(dstDrawableId) after draw has 0 non-white pixels!")
        }
    }

    // Release destination context
    if dstIsWindow != 0 {
        backend.releaseWindowContext(id: Int(dstDrawableId))
    }

    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_flush")
public func macos_backend_flush(_ handle: BackendHandle) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    // Use a semaphore to wait for the async work to complete
    // This ensures display updates happen before returning, without deadlocking
    let semaphore = DispatchSemaphore(value: 0)

    DispatchQueue.main.async {
        NSLog("flush: updating \(backend.windowContentViews.count) windows")

        // Update all content views
        for (id, contentView) in backend.windowContentViews {
            if let buffer = backend.windowBuffers[id], let window = backend.windows[id] {
                guard buffer.context?.makeImage() != nil else {
                    NSLog("flush: window \(id) - no CGImage available!")
                    continue
                }

                NSLog("flush: window \(id) - contentView frame: \(contentView.frame)")

                // Update the image view content
                contentView.updateContents()
                window.display()
            }
        }

        semaphore.signal()
    }

    // Wait with a timeout to avoid deadlock if main thread is blocked
    _ = semaphore.wait(timeout: .now() + 0.5)

    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_draw_text")
public func macos_backend_draw_text(_ handle: BackendHandle, isWindow: Int32, drawableId: Int32,
                                    x: Int32, y: Int32,
                                    text: UnsafePointer<CChar>,
                                    r: Float, g: Float, b: Float) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    let textStr = String(cString: text)

    NSLog("draw_text: isWindow=\(isWindow), drawable=\(drawableId), pos=(\(x),\(y)), text='\(textStr)', color=(\(r),\(g),\(b))")

    let context: CGContext?
    if isWindow != 0 {
        context = backend.getWindowContext(id: Int(drawableId))
    } else {
        context = backend.getPixmapContext(id: Int(drawableId))
    }

    guard let ctx = context else {
        NSLog("draw_text: ERROR - no context!")
        return BackendResult.error.rawValue
    }

    // Save graphics state
    ctx.saveGState()

    // Create attributed string with a fixed-width font
    let font = CTFontCreateWithName("Menlo" as CFString, 12, nil)
    let attributes: [NSAttributedString.Key: Any] = [
        .font: font,
        .foregroundColor: NSColor(red: CGFloat(r), green: CGFloat(g), blue: CGFloat(b), alpha: 1)
    ]
    let attributedString = NSAttributedString(string: textStr, attributes: attributes)

    // Create line from attributed string
    let line = CTLineCreateWithAttributedString(attributedString)

    // The context has a CTM that flips Y for X11 compatibility.
    // Core Text draws text with its own coordinate expectations, so we need to
    // locally flip the text drawing to make it right-side up.
    // Move to the text position, then flip just around that point
    ctx.translateBy(x: CGFloat(x), y: CGFloat(y))
    ctx.scaleBy(x: 1.0, y: -1.0)
    ctx.textPosition = CGPoint(x: 0, y: 0)
    CTLineDraw(line, ctx)

    // Restore graphics state
    ctx.restoreGState()

    if isWindow != 0 {
        backend.releaseWindowContext(id: Int(drawableId))
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

            // Find which window this event is for and get content height for Y coordinate flipping
            var contentHeight: CGFloat = 0
            if let eventWindow = nsEvent.window {
                for (id, window) in backend.windows {
                    if window === eventWindow {
                        evtWindowId = Int32(id)
                        // Get content view height for Y coordinate conversion
                        if let contentView = window.contentView {
                            contentHeight = contentView.bounds.height
                        }
                        break
                    }
                }
            }

            // Helper to convert macOS Y (origin at bottom) to X11 Y (origin at top)
            func flipY(_ macY: CGFloat) -> Int32 {
                return Int32(contentHeight - macY)
            }

            evtTime = Int32(nsEvent.timestamp * 1000) // Convert to milliseconds

            switch nsEvent.type {
            case .leftMouseDown:
                evtType = 5 // buttonpress
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .rightMouseDown:
                evtType = 5 // buttonpress
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .otherMouseDown:
                evtType = 5 // buttonpress
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .leftMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .rightMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .otherMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
                evtType = 7 // motion
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseEntered:
                evtType = 10 // enterNotify
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseExited:
                evtType = 11 // leaveNotify
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

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
            // Find which window this event is for and get content height for Y coordinate flipping
            var contentHeight: CGFloat = 0
            if let eventWindow = nsEvent.window {
                for (id, window) in backend.windows {
                    if window === eventWindow {
                        evtWindowId = Int32(id)
                        // Get content view height for Y coordinate conversion
                        if let contentView = window.contentView {
                            contentHeight = contentView.bounds.height
                        }
                        break
                    }
                }
            }

            // Helper to convert macOS Y (origin at bottom) to X11 Y (origin at top)
            func flipY(_ macY: CGFloat) -> Int32 {
                return Int32(contentHeight - macY)
            }

            evtTime = Int32(nsEvent.timestamp * 1000) // Convert to milliseconds

            switch nsEvent.type {
            case .leftMouseDown:
                evtType = 5 // buttonpress
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .rightMouseDown:
                evtType = 5 // buttonpress
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .otherMouseDown:
                evtType = 5 // buttonpress
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .leftMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 1
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .rightMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 3
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .otherMouseUp:
                evtType = 6 // buttonrelease
                evtButton = 2
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged:
                evtType = 7 // motion
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseEntered:
                evtType = 10 // enterNotify
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

            case .mouseExited:
                evtType = 11 // leaveNotify
                evtX = Int32(nsEvent.locationInWindow.x)
                evtY = flipY(nsEvent.locationInWindow.y)

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

// MARK: - Cursor Operations

/// Convert cursor type to NSCursor
func cursorTypeToNSCursor(_ cursorType: Int) -> NSCursor {
    switch cursorType {
    case 0: return NSCursor.arrow
    case 1: return NSCursor.iBeam
    case 2: return NSCursor.crosshair
    case 3: return NSCursor.pointingHand
    case 4: return NSCursor.closedHand
    case 5: return NSCursor.resizeLeftRight
    case 6: return NSCursor.resizeUpDown
    case 7: return NSCursor.operationNotAllowed
    case 8: return NSCursor.operationNotAllowed
    default: return NSCursor.arrow
    }
}

@_cdecl("macos_backend_create_cursor")
public func macos_backend_create_cursor(_ handle: BackendHandle, cursorType: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let id = backend.nextId
    backend.nextId += 1

    let cursor = cursorTypeToNSCursor(Int(cursorType))
    backend.cursors[id] = cursor

    return Int32(id)
}

@_cdecl("macos_backend_free_cursor")
public func macos_backend_free_cursor(_ handle: BackendHandle, cursorId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()
    backend.cursors.removeValue(forKey: Int(cursorId))
    return BackendResult.success.rawValue
}

@_cdecl("macos_backend_set_window_cursor")
public func macos_backend_set_window_cursor(_ handle: BackendHandle, windowId: Int32, cursorId: Int32) -> Int32 {
    let backend = Unmanaged<MacOSBackendImpl>.fromOpaque(handle).takeUnretainedValue()

    let cursor: NSCursor
    if cursorId == 0 {
        cursor = NSCursor.arrow
    } else if let c = backend.cursors[Int(cursorId)] {
        cursor = c
    } else {
        return BackendResult.error.rawValue
    }

    backend.windowCursors[Int(windowId)] = cursor

    // Set the cursor if the window is key
    DispatchQueue.main.async {
        if let window = backend.windows[Int(windowId)], window.isKeyWindow {
            cursor.set()
        }
    }

    return BackendResult.success.rawValue
}
