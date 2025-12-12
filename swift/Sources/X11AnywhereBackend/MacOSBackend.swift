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

// MARK: - Application Delegate

class X11AppDelegate: NSObject, NSApplicationDelegate {
    static let shared = X11AppDelegate()
    private var eventMonitor: Any?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("X11AppDelegate: applicationDidFinishLaunching")

        // Add local event monitor to debug mouse/keyboard events
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .keyDown]) { event in
            NSLog("LocalEventMonitor: type=\(event.type.rawValue), window=\(event.window?.title ?? "nil")")
            return event
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        NSLog("X11AppDelegate: applicationWillTerminate")
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        // Don't quit when windows are closed
        return false
    }
}

// MARK: - Event Queue for X11 Events

/// Structure representing an X11 event queued from NSView handlers
struct X11QueuedEvent {
    var type: Int32       // X11 event type
    var windowId: Int32   // Backend window ID
    var x: Int32          // X coordinate
    var y: Int32          // Y coordinate
    var width: Int32      // Width (for configure events)
    var height: Int32     // Height (for configure events)
    var keycode: Int32    // Key code
    var button: Int32     // Mouse button
    var state: Int32      // Modifier state
    var time: Int32       // Event timestamp in milliseconds
}

/// Global event queue that NSViews can write to and poll_event can read from
/// Access must be synchronized since NSViews run on main thread and poll_event may be called from worker thread
class X11EventQueue {
    static let shared = X11EventQueue()

    private var events: [X11QueuedEvent] = []
    private let lock = NSLock()

    func enqueue(_ event: X11QueuedEvent) {
        lock.lock()
        defer { lock.unlock() }
        events.append(event)
        NSLog("X11EventQueue: enqueued event type=\(event.type), window=\(event.windowId), x=\(event.x), y=\(event.y), keycode=\(event.keycode)")
    }

    func dequeue() -> X11QueuedEvent? {
        lock.lock()
        defer { lock.unlock() }
        if events.isEmpty {
            return nil
        }
        return events.removeFirst()
    }

    var isEmpty: Bool {
        lock.lock()
        defer { lock.unlock() }
        return events.isEmpty
    }
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
    var windowId: Int32 = 0  // Backend window ID for event routing
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

    // Accept keyboard events
    override var acceptsFirstResponder: Bool { return true }
    override var canBecomeKeyView: Bool { return true }

    /// Convert macOS keycode to X11 keycode
    /// X11 keycodes are typically offset by 8 from hardware scancodes
    private func macToX11Keycode(_ macKeycode: UInt16) -> Int32 {
        // macOS keycodes are similar to hardware scancodes
        // X11 keycodes = hardware scancode + 8
        return Int32(macKeycode) + 8
    }

    /// Get current modifier state as X11 modifier mask
    private func modifierState(from event: NSEvent) -> Int32 {
        var state: Int32 = 0
        let flags = event.modifierFlags

        if flags.contains(.shift) { state |= 1 }     // ShiftMask
        if flags.contains(.control) { state |= 4 }   // ControlMask
        if flags.contains(.option) { state |= 8 }    // Mod1Mask (Alt)
        if flags.contains(.command) { state |= 64 }  // Mod4Mask (Super)
        if flags.contains(.capsLock) { state |= 2 }  // LockMask

        return state
    }

    /// Convert macOS Y coordinate (origin at bottom-left) to X11 Y (origin at top-left)
    /// NOTE: Since isFlipped = true, coordinates from convert() are already in top-left origin,
    /// so we just cast to Int32 without flipping.
    private func convertY(_ macY: CGFloat) -> Int32 {
        // With isFlipped=true, Y is already correct (0 at top)
        return Int32(macY)
    }

    /// Get timestamp in milliseconds
    private func eventTime(_ event: NSEvent) -> Int32 {
        return Int32(event.timestamp * 1000)
    }

    override func keyDown(with event: NSEvent) {
        // Don't call super - we handle it ourselves and don't want the beep
        NSLog("X11ContentView.keyDown: keyCode=\(event.keyCode), chars=\(event.characters ?? "nil")")

        let queuedEvent = X11QueuedEvent(
            type: 2,  // X11 KeyPress
            windowId: windowId,
            x: 0, y: 0, width: 0, height: 0,
            keycode: macToX11Keycode(event.keyCode),
            button: 0,
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func keyUp(with event: NSEvent) {
        NSLog("X11ContentView.keyUp: keyCode=\(event.keyCode)")

        let queuedEvent = X11QueuedEvent(
            type: 3,  // X11 KeyRelease
            windowId: windowId,
            x: 0, y: 0, width: 0, height: 0,
            keycode: macToX11Keycode(event.keyCode),
            button: 0,
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseDown(with event: NSEvent) {
        NSLog("X11ContentView.mouseDown: location=\(event.locationInWindow)")
        // Make sure we become first responder on click
        window?.makeFirstResponder(self)

        // locationInWindow is in window coordinates (Y=0 at bottom)
        // We need to convert to X11 coordinates (Y=0 at top)
        let loc = event.locationInWindow
        let x = Int32(loc.x)
        // Convert Y from bottom-up (macOS) to top-down (X11)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 4,  // X11 ButtonPress
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0,
            button: 1,  // Left button
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseUp(with event: NSEvent) {
        NSLog("X11ContentView.mouseUp: location=\(event.locationInWindow)")

        let loc = event.locationInWindow
        let x = Int32(loc.x)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 5,  // X11 ButtonRelease
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0,
            button: 1,  // Left button
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func rightMouseDown(with event: NSEvent) {
        NSLog("X11ContentView.rightMouseDown: location=\(event.locationInWindow)")

        let loc = event.locationInWindow
        let x = Int32(loc.x)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 4,  // X11 ButtonPress
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0,
            button: 3,  // Right button
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func rightMouseUp(with event: NSEvent) {
        NSLog("X11ContentView.rightMouseUp: location=\(event.locationInWindow)")

        let loc = event.locationInWindow
        let x = Int32(loc.x)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 5,  // X11 ButtonRelease
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0,
            button: 3,  // Right button
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseMoved(with event: NSEvent) {
        let loc = event.locationInWindow
        let x = Int32(loc.x)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 6,  // X11 MotionNotify
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0, button: 0,
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseDragged(with event: NSEvent) {
        let loc = event.locationInWindow
        let x = Int32(loc.x)
        let windowHeight = window?.contentView?.bounds.height ?? bounds.height
        let y = Int32(windowHeight - loc.y)

        let queuedEvent = X11QueuedEvent(
            type: 6,  // X11 MotionNotify
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0,
            button: 1,  // Left button held
            state: modifierState(from: event) | 256,  // Button1Mask
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseEntered(with event: NSEvent) {
        NSLog("X11ContentView.mouseEntered: windowId=\(windowId)")
        let loc = convert(event.locationInWindow, from: nil)
        let x = Int32(loc.x)
        let y = Int32(loc.y)  // Already flipped because isFlipped=true

        let queuedEvent = X11QueuedEvent(
            type: 10,  // X11 EnterNotify
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0, button: 0,
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func mouseExited(with event: NSEvent) {
        NSLog("X11ContentView.mouseExited: windowId=\(windowId)")
        let loc = convert(event.locationInWindow, from: nil)
        let x = Int32(loc.x)
        let y = Int32(loc.y)  // Already flipped because isFlipped=true

        let queuedEvent = X11QueuedEvent(
            type: 11,  // X11 LeaveNotify
            windowId: windowId,
            x: x,
            y: y,
            width: 0, height: 0, keycode: 0, button: 0,
            state: modifierState(from: event),
            time: eventTime(event)
        )
        X11EventQueue.shared.enqueue(queuedEvent)
    }

    override func becomeFirstResponder() -> Bool {
        NSLog("X11ContentView.becomeFirstResponder: windowId=\(windowId)")
        // Use system uptime (like NSEvent.timestamp) to avoid overflow
        let time = Int32(truncatingIfNeeded: Int64(ProcessInfo.processInfo.systemUptime * 1000))
        let queuedEvent = X11QueuedEvent(
            type: 8,  // X11 FocusIn
            windowId: windowId,
            x: 0, y: 0, width: 0, height: 0,
            keycode: 0, button: 0, state: 0,
            time: time
        )
        X11EventQueue.shared.enqueue(queuedEvent)
        return super.becomeFirstResponder()
    }

    override func resignFirstResponder() -> Bool {
        NSLog("X11ContentView.resignFirstResponder: windowId=\(windowId)")
        // Use system uptime (like NSEvent.timestamp) to avoid overflow
        let time = Int32(truncatingIfNeeded: Int64(ProcessInfo.processInfo.systemUptime * 1000))
        let queuedEvent = X11QueuedEvent(
            type: 9,  // X11 FocusOut
            windowId: windowId,
            x: 0, y: 0, width: 0, height: 0,
            keycode: 0, button: 0, state: 0,
            time: time
        )
        X11EventQueue.shared.enqueue(queuedEvent)
        return super.resignFirstResponder()
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        guard let buffer = self.buffer, let ctx = buffer.context, let cgImage = ctx.makeImage() else {
            return
        }

        guard let currentCtx = NSGraphicsContext.current?.cgContext else {
            return
        }

        // Get the title bar height by comparing window frame to content layout rect
        // contentLayoutRect excludes the title bar area
        var titleBarHeight: CGFloat = 0
        if let window = self.window {
            titleBarHeight = window.frame.height - window.contentLayoutRect.height
        }

        let imageWidth = CGFloat(cgImage.width)
        let imageHeight = CGFloat(cgImage.height)
        currentCtx.draw(cgImage, in: CGRect(x: 0, y: titleBarHeight, width: imageWidth, height: imageHeight))
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

        // Set up a delegate to track events
        if app.delegate == nil {
            app.delegate = X11AppDelegate.shared
            NSLog("initializeOnMainThread: set app delegate")
        }

        // Finish launching the application so it can receive events
        if !app.isRunning {
            app.finishLaunching()
            NSLog("initializeOnMainThread: called finishLaunching()")
        }

        // Activate the app to bring it to front and allow it to receive events
        app.activate(ignoringOtherApps: true)
        NSLog("initializeOnMainThread: activated app")

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
            contentView.windowId = Int32(id)  // Set the window ID for event routing

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
                    // Make content view first responder for keyboard events
                    window.makeFirstResponder(contentView)
                    NSLog("mapWindow: updated contentView, buffer context: \(contentView.buffer?.context != nil), firstResponder: \(window.firstResponder === contentView)")
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
    // Read from the event queue that NSView handlers write to
    // This works because NSApplication.run() dispatches events to views,
    // which then queue them for us to read here
    if let event = X11EventQueue.shared.dequeue() {
        NSLog("poll_event: dequeued event type=\(event.type), window=\(event.windowId), x=\(event.x), y=\(event.y), keycode=\(event.keycode)")
        eventType.pointee = event.type
        windowId.pointee = event.windowId
        x.pointee = event.x
        y.pointee = event.y
        width.pointee = event.width
        height.pointee = event.height
        keycode.pointee = event.keycode
        button.pointee = event.button
        state.pointee = event.state
        time.pointee = event.time
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
    // Wait for an event by polling the queue with small sleeps
    // This is a blocking call that waits until an event is available
    while true {
        if let event = X11EventQueue.shared.dequeue() {
            NSLog("wait_for_event: dequeued event type=\(event.type), window=\(event.windowId)")
            eventType.pointee = event.type
            windowId.pointee = event.windowId
            x.pointee = event.x
            y.pointee = event.y
            width.pointee = event.width
            height.pointee = event.height
            keycode.pointee = event.keycode
            button.pointee = event.button
            state.pointee = event.state
            time.pointee = event.time
            return 1 // Has event
        }
        // Sleep briefly to avoid busy-waiting
        Thread.sleep(forTimeInterval: 0.01)
    }
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

// MARK: - Run Loop

/// Run the NSApplication event loop. This function never returns.
/// Call this from the main thread instead of CFRunLoopRun() to ensure
/// that NSApplication events are properly processed.
@_cdecl("macos_backend_run_app")
public func macos_backend_run_app() {
    NSLog("macos_backend_run_app: starting NSApplication.run()")
    let app = NSApplication.shared

    // Make sure the app is properly set up
    if app.delegate == nil {
        app.delegate = X11AppDelegate.shared
    }

    if !app.isRunning {
        app.finishLaunching()
    }

    app.activate(ignoringOtherApps: true)

    // Run the application - this will block and process events
    app.run()
}
