import XCTest
@testable import PortRelayUSB

func fixtureDevice(entry: UInt64 = 1, location: UInt32 = 0x00123000, speed: Int = 2,
                deviceClass: UInt8 = 0, interfaces: [InterfaceDescription]? = nil) -> RegistryDevice {
        RegistryDevice(entryID: entry, location: location, vendor: 0x1234, product: 0x5678,
                       deviceClass: deviceClass, subClass: 0, deviceProtocol: 0, rawSpeed: speed,
                       name: "Protocol fixture, not hardware", interfaces: interfaces ?? [.init(number: 0, deviceClass: 255, drivers: [])])
    }
final class BoundaryTests: XCTestCase {
    func testFirstMacBusAndHubPathHaveAnUnambiguousWireIdentity() {
        let d = fixtureDevice()
        XCTAssertEqual(d.id, "1-1.2.3")
        XCTAssertEqual(d.devid, 0x00010123)
        XCTAssertNil(d.relay(boot: "boot").blocked)
        XCTAssertNotEqual(d.devid, fixtureDevice(location: 0x00300000).devid)
        XCTAssertNotEqual(d.devid, fixtureDevice(location: 0x01123000).devid)
        XCTAssertEqual(d.usb.registryEntryID, 1)
    }
    func testReplacementAndRebootRevokeOldGenerationEvenWithIdenticalVIDPID() {
        let first = fixtureDevice().relay(boot: "a")
        XCTAssertEqual(first.generation.count, 64)
        XCTAssertEqual(first.generation, fixtureDevice().relay(boot: "a").generation)
        XCTAssertNotEqual(first.generation, fixtureDevice(entry: 2).relay(boot: "a").generation)
        XCTAssertNotEqual(first.generation, fixtureDevice().relay(boot: "b").generation)
    }
    func testConservativePolicyNeverDetachesDriversOrExportsUnsupportedInterfaces() {
        for d in [fixtureDevice(speed: -1), fixtureDevice(location: 0), fixtureDevice(location: 0x00123450),
                  fixtureDevice(deviceClass: 9), fixtureDevice(interfaces: []),
                  fixtureDevice(interfaces: [.init(number: 1, deviceClass: 255, drivers: [])]),
                  fixtureDevice(interfaces: [.init(number: 0, deviceClass: 255, drivers: ["ExistingDriver"])]),
                  fixtureDevice(interfaces: [.init(number: 0, deviceClass: 255, drivers: []), .init(number: 1, deviceClass: 255, drivers: [])])] {
            XCTAssertNotNil(d.relay(boot: "a").blocked)
        }
        for usbClass in [3, 8, 9, 0xe0, 1, 2, 0x0e] {
            XCTAssertNotNil(fixtureDevice(interfaces: [.init(number: 0, deviceClass: usbClass, drivers: [])]).relay(boot: "a").blocked)
        }
    }
    func header(command: UInt32 = 1, devid: UInt32 = 65537, direction: UInt32 = 0,
                endpoint: UInt32 = 1, length: UInt32 = 3, packets: UInt32 = 0) -> Data {
        var data = Data(repeating: 0, count: 48)
        for (offset, value) in [(0,command),(4,1),(8,devid),(12,direction),(16,endpoint),(20,2),(24,length),(32,packets)] {
            var value = value.bigEndian
            withUnsafeBytes(of: &value) { data.replaceSubrange(offset..<offset+4, with: $0) }
        }
        return data
    }
    func testFramingReadsPayloadOnlyForOutTransfers() throws {
        XCTAssertEqual(try bodyLength(header(), expectedDevice: 65537), 3)
        XCTAssertEqual(try bodyLength(header(direction: 1), expectedDevice: 65537), 0)
        XCTAssertEqual(try bodyLength(header(command: 2), expectedDevice: 65537), 0)
    }
    func testForeignDevicesAndMalformedRequestsAreRejectedBeforeUSBAccess() {
        for data in [Data(), header(devid: 7), header(command: 3), header(direction: 2),
                     header(endpoint: 16), header(length: 1_048_577), header(length: .max), header(packets: 1)] {
            XCTAssertThrowsError(try bodyLength(data, expectedDevice: 65537))
        }
    }
    func testTruncatedInputIsDifferentFromCleanParentExit() throws {
        let pipe = Pipe()
        try pipe.fileHandleForWriting.write(contentsOf: Data([1,2,3]))
        try pipe.fileHandleForWriting.close()
        XCTAssertThrowsError(try readExactly(48, from: pipe.fileHandleForReading))
        XCTAssertNil(try readExactly(48, from: pipe.fileHandleForReading))
    }
}
