// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "X11AnywhereBackend",
    platforms: [
        .macOS(.v10_15)
    ],
    products: [
        .library(
            name: "X11AnywhereBackend",
            type: .static,
            targets: ["X11AnywhereBackend"]
        ),
    ],
    targets: [
        .target(
            name: "X11AnywhereBackend",
            dependencies: []
        ),
    ]
)
