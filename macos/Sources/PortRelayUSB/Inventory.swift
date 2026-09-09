import Foundation
import IOKit
import CryptoKit
import Common

struct RelayError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}
struct RelayDevice: Codable {
    var id: String
    var generation: String
    var name: String
    var vendor: String
    var product: String
    var kind: String
    var speed: UInt32
    var devid: UInt32
    var blocked: String?
    var risks: [String] = []
    var parent_hub: String?
}
struct InterfaceDescription {
    let number: Int
    let deviceClass: Int
    let drivers: [String]
}
struct RegistryDevice {
    let entryID: UInt64
    let location: UInt32
    let vendor: UInt16
    let product: UInt16
    let deviceClass: UInt8
    let subClass: UInt8
    let deviceProtocol: UInt8
    let rawSpeed: Int
    let name: String
    let interfaces: [InterfaceDescription]

    var bus: String { String((location >> 24) + 1) }
    var ports: [UInt32] {
        var values = stride(from: 20, through: 0, by: -4).map { (location >> UInt32($0)) & 15 }
        while values.last == 0 { values.removeLast() }
        return values
    }
    var path: String { ports.map(String.init).joined(separator: ".") }
    var id: String { "\(bus)-\(path)" }
    var devid: UInt32 { (((location >> 24) + 1) << 16) | ports.reduce(0) { ($0 << 4) | $1 } }
    var usb: USBDevice {
        USBDevice(busID: bus, deviceID: path, vendorID: vendor, productID: product,
                  deviceClass: deviceClass, deviceSubClass: subClass, deviceProtocol: deviceProtocol,
                  speed: rawSpeed >= 3 ? .superSpeed : rawSpeed == 2 ? .high : rawSpeed == 1 ? .full : .low,
                  manufacturerString: nil, productString: name, serialNumberString: nil,
                  registryEntryID: entryID)
    }
    func relay(boot: String) -> RelayDevice {
        let digest = SHA256.hash(data: Data("\(boot):\(entryID):\(location):\(vendor):\(product)".utf8))
        let generation = digest.map { String(format: "%02x", $0) }.joined()
        let classes = Set(interfaces.map(\.deviceClass) + [Int(deviceClass)])
        let kind = classes.contains(9) ? "hub" : classes.contains(8) ? "storage" : classes.contains(3) ? "input" : classes.contains(0xe0) ? "bluetooth" : "usb"
        let reason: String?
        if !(0...4).contains(rawSpeed) {
            reason = "This USB speed is not supported by the Mac backend."
        } else if ports.isEmpty || ports.count > 4 || ports.contains(0) {
            reason = "This USB port topology is not supported by the Mac backend."
        } else if classes.contains(9) {
            reason = "Select a device connected to this hub; the hub itself cannot be shared."
        } else if interfaces.count != 1 || interfaces.first?.number != 0 {
            reason = "The first Mac exporter supports one USB interface (number 0). Composite devices need further integration."
        } else if ![0, 0xff, 0xfe].contains(Int(deviceClass)) || ![0xff, 0xfe].contains(interfaces[0].deviceClass) {
            reason = "This USB class is not enabled in the first Mac exporter. Disk, HID, network, audio and Bluetooth driver handoff needs separate support."
        } else if !interfaces[0].drivers.isEmpty {
            reason = "macOS or another app owns this interface: \(interfaces[0].drivers.joined(separator: ", ")). Close its app; driver-owned interfaces are not detached."
        } else {
            reason = nil
        }
        // IOKit USB Speed is 0=low, 1=full, 2=high, 3+=SuperSpeed. USB/IP uses 5 for SuperSpeed.
        return RelayDevice(id: id, generation: generation, name: name, vendor: String(format: "%04x", vendor), product: String(format: "%04x", product), kind: kind, speed: rawSpeed < 0 ? 0 : rawSpeed >= 3 ? 5 : UInt32(rawSpeed + 1), devid: devid, blocked: reason, parent_hub: ports.count > 1 ? "\(bus)-\(ports.dropLast().map(String.init).joined(separator: "."))" : nil)
    }
}
func property(_ entry: io_registry_entry_t, _ name: String) -> Any? {
    IORegistryEntryCreateCFProperty(entry, name as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue()
}
func intProperty(_ entry: io_registry_entry_t, _ name: String) -> Int? { (property(entry, name) as? NSNumber)?.intValue }
func children(_ entry: io_registry_entry_t, _ body: (io_registry_entry_t) -> Void) {
    var iterator: io_iterator_t = 0
    guard IORegistryEntryGetChildIterator(entry, kIOServicePlane, &iterator) == KERN_SUCCESS else { return }
    defer { IOObjectRelease(iterator) }
    while true {
        let child = IOIteratorNext(iterator)
        if child == 0 { break }
        body(child)
        IOObjectRelease(child)
    }
}
func interfaces(_ entry: io_registry_entry_t, depth: Int = 0) -> [InterfaceDescription] {
    if depth > 4 { return [] }
    var result: [InterfaceDescription] = []
    children(entry) { child in
        if IOObjectConformsTo(child, "IOUSBHostInterface") != 0 || IOObjectConformsTo(child, "IOUSBInterface") != 0 {
            var drivers: [String] = []
            children(child) { owner in
                if let name = IOObjectCopyClass(owner)?.takeRetainedValue() as String? {
                    // Ignore only the compatibility bridge. Never open an interface while listing it.
                    if !["IOUSBHostLegacyClient", "AppleUSBHostLegacyClient"].contains(name) { drivers.append(name) }
                }
            }
            result.append(InterfaceDescription(number: intProperty(child, "bInterfaceNumber") ?? -1, deviceClass: intProperty(child, "bInterfaceClass") ?? -1, drivers: drivers))
        } else if IOObjectConformsTo(child, "IOUSBHostDevice") == 0 {
            result += interfaces(child, depth: depth + 1)
        }
    }
    return result
}
func bootID() throws -> String {
    var count = 0
    guard sysctlbyname("kern.bootsessionuuid", nil, &count, nil, 0) == 0, count > 1, count < 128 else { throw RelayError("Cannot establish the Mac boot identity") }
    var buffer = [CChar](repeating: 0, count: count)
    guard sysctlbyname("kern.bootsessionuuid", &buffer, &count, nil, 0) == 0 else { throw RelayError("Cannot read the Mac boot identity") }
    return String(cString: buffer)
}
func inventory() throws -> [RegistryDevice] {
    var iterator: io_iterator_t = 0
    guard let matching = IOServiceMatching("IOUSBHostDevice"),
          IOServiceGetMatchingServices(kIOMainPortDefault, matching, &iterator) == KERN_SUCCESS else { throw RelayError("USB inventory is unavailable") }
    defer { IOObjectRelease(iterator) }
    var result: [RegistryDevice] = []
    while true {
        let entry = IOIteratorNext(iterator)
        if entry == 0 { break }
        defer { IOObjectRelease(entry) }
        var id: UInt64 = 0
        guard IORegistryEntryGetRegistryEntryID(entry, &id) == KERN_SUCCESS,
              let location = intProperty(entry, "locationID"), let vendor = intProperty(entry, "idVendor"), let product = intProperty(entry, "idProduct") else { continue }
        result.append(RegistryDevice(entryID: id, location: UInt32(truncatingIfNeeded: location), vendor: UInt16(truncatingIfNeeded: vendor), product: UInt16(truncatingIfNeeded: product), deviceClass: UInt8(truncatingIfNeeded: intProperty(entry, "bDeviceClass") ?? 0), subClass: UInt8(truncatingIfNeeded: intProperty(entry, "bDeviceSubClass") ?? 0), deviceProtocol: UInt8(truncatingIfNeeded: intProperty(entry, "bDeviceProtocol") ?? 0), rawSpeed: intProperty(entry, "Device Speed") ?? -1, name: property(entry, "USB Product Name") as? String ?? String(format: "USB %04x:%04x", vendor, product), interfaces: interfaces(entry)))
    }
    return result.sorted { $0.id < $1.id }
}
