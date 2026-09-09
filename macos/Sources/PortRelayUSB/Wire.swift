import Foundation
import Common
import USBIPDCore

let maximumTransfer = 1_048_576
func u32(_ data: Data, _ offset: Int) -> UInt32 {
    data[offset..<offset+4].reduce(0) { ($0 << 8) | UInt32($1) }
}
func readExactly(_ count: Int, from handle: FileHandle = .standardInput) throws -> Data? {
    var data = Data()
    while data.count < count {
        guard let chunk = try handle.read(upToCount: count - data.count), !chunk.isEmpty else {
            if data.isEmpty { return nil }
            throw RelayError("Truncated USB/IP request")
        }
        data.append(chunk)
    }
    return data
}
func bodyLength(_ header: Data, expectedDevice: UInt32) throws -> Int {
    guard header.count == 48, [1, 2].contains(u32(header, 0)), u32(header, 8) == expectedDevice,
          u32(header, 12) <= 1, u32(header, 16) <= 15 else { throw RelayError("USB/IP request does not match the selected device") }
    if u32(header, 0) == 2 {
        guard u32(header, 20) != 0, u32(header, 20) != u32(header, 4) else { throw RelayError("Invalid cancellation sequence") }
        return 0
    }
    guard u32(header, 24) <= maximumTransfer, u32(header, 32) == 0 else { throw RelayError("Oversized or isochronous transfer is not supported") }
    return u32(header, 12) == 0 ? Int(u32(header, 24)) : 0
}
func frame<T: Encodable>(_ value: T) throws {
    let body = try JSONEncoder().encode(value)
    guard body.count <= 65_536 else { throw RelayError("Local reply is too large") }
    var size = UInt32(body.count).bigEndian
    try FileHandle.standardOutput.write(contentsOf: Data(bytes: &size, count: 4) + body)
}
struct Ready: Encodable {
    let error: String?
    let device: RelayDevice?
    let port: UInt32? = nil
}
final class SelectedDiscovery: DeviceDiscovery {
    let device: USBDevice
    var onDeviceConnected: ((USBDevice) -> Void)?
    var onDeviceDisconnected: ((USBDevice) -> Void)?
    init(_ device: USBDevice) { self.device = device }
    func discoverDevices() throws -> [USBDevice] { [device] }
    func getDevice(busID: String, deviceID: String) throws -> USBDevice? { busID == device.busID && deviceID == device.deviceID ? device : nil }
    func startNotifications() throws {}
    func stopNotifications() {}
}
