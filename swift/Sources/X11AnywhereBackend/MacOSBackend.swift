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

// MARK: - Custom View with Backing Buffer

class X11BackingView: NSView {
    var backingImage: NSImage?
    var backingContext: CGContext?
    var viewWidth: Int = 100
    var viewHeight: Int = 100

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupBacking(width: Int(frameRect.width), height: Int(frameRect.height))
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
    }

    func setupBacking(width: Int, height: Int) {
        self.viewWidth = max(width, 1)
        self.viewHeight = max(height, 1)

        // Create a bitmap context for our backing store
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue

        if let ctx = CGContext(data: nil,
                               width: viewWidth,
                               height: viewHeight,
                               bitsPerComponent: 8,
                               bytesPerRow: viewWidth * 4,
                               space: colorSpace,
                               bitmapInfo: bitmapInfo) {
            // Fill with white background
            ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
            ctx.fill(CGRect(x: 0, y: 0, width: viewWidth, height: viewHeight))
            self.backingContext = ctx
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext,
              let backingCtx = backingContext,
              let image = backingCtx.makeImage() else {
            // Fill with white if no backing
            NSColor.white.setFill()
            dirtyRect.fill()
            return
        }

        // Draw the backing image to the view
        // X11 coordinates: origin at top-left, Y increases downward
        // NSView coordinates: origin at bottom-left, Y increases upward
        // CGContext backing: origin at bottom-left
        // We need to flip when drawing from backing to view
        ctx.saveGState()
        ctx.translateBy(x: 0, y: bounds.height)
        ctx.scaleBy(x: 1, y: -1)
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: CGFloat(viewWidth), height: CGFloat(viewHeight)))
        ctx.restoreGState()
    }

    func getContext() -> CGContext? {
        return backingContext
    }
}

// MARK: - Backend Class

class MacOSBackendImpl {
    var windows: [Int: NSWindow] = [:]
    var windowViews: [Int: X11BackingView] = [:]
    var contexts: [Int: CGContext] = [:]
    var nextId: Int = 1
    var screenWidth: Int = 1920  // Default fallback
    var screenHeight: Int = 1080 // Default fallback
    var screenWidthMM: Int = 508
    var screenHeightMM: Int = 285

    init() {
        // Initialize NSApplication first (required for Cocoa operations)
        let initBlock = { [self] in
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
            // If NSScreen.main is nil, we keep the default values
        }

        // Run on main thread without deadlock
        if Thread.isMainThread {
            initBlock()
        } else {
            DispatchQueue.main.sync {
                initBlock()
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

            // Create our custom backing view
            let backingView = X11BackingView(frame: NSRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height)))
            window.contentView = backingView

            self.windows[id] = window
            self.windowViews[id] = backingView
            windowId = id
        }
        return windowId
    }

    func destroyWindow(id: Int) {
        DispatchQueue.main.sync {
            self.windowViews.removeValue(forKey: id)
            if let window = self.windows.removeValue(forKey: id) {
                window.close()
            }
        }
    }

    func mapWindow(id: Int) {
        DispatchQueue.main.sync {
            if let window = self.windows[id] {
                // Activate the app first so windows can be shown
                NSApplication.shared.activate(ignoringOtherApps: true)
                window.makeKeyAndOrderFront(nil)
                // Force an initial display
                if let contentView = window.contentView {
                    contentView.setNeedsDisplay(contentView.bounds)
                }
                window.displayIfNeeded()

                // Pump the run loop briefly to ensure the window actually appears
                // This is necessary for the window to be visible for drawing operations
                RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
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
        // Return the backing context from our custom view
        return self.windowViews[id]?.getContext()
    }

    func releaseWindowContext(id: Int) {
        // Mark the view as needing display to show the changes
        DispatchQueue.main.sync {
            if let view = self.windowViews[id] {
                view.setNeedsDisplay(view.bounds)
                view.displayIfNeeded()
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
        // Force all windows to display their content
        for (_, view) in backend.windowViews {
            view.setNeedsDisplay(view.bounds)
            view.displayIfNeeded()
        }
        for (_, window) in backend.windows {
            window.displayIfNeeded()
        }
        NSApplication.shared.updateWindows()

        // Pump the run loop to process display updates
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
    }
    return BackendResult.success.rawValue
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
