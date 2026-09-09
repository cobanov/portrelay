// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "PortRelayMac",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "portrelay-macos-usb", targets: ["PortRelayUSB"])],
    dependencies: [.package(path: "../target/macos/usbipd-mac-pinned")],
    targets: [
        .executableTarget(name: "PortRelayUSB", dependencies: [
            .product(name: "USBIPDCore", package: "usbipd-mac-pinned"),
            .product(name: "Common", package: "usbipd-mac-pinned")
        ], linkerSettings: [.linkedFramework("IOKit")]),
        .testTarget(name: "PortRelayUSBTests", dependencies: ["PortRelayUSB"])
    ],
    swiftLanguageModes: [.v5]
)
