import XCTest
import Common
import USBIPDCore
@testable import PortRelayUSB

private actor Gate {
    var opened = false
    var waiters: [CheckedContinuation<Void, Never>] = []
    func wait() async { if !opened { await withCheckedContinuation { waiters.append($0) } } }
    func release() { opened = true; waiters.forEach { $0.resume() }; waiters.removeAll() }
}
private final class FixtureUSB: USBDeviceCommunicator, @unchecked Sendable {
    let gate = Gate()
    let started: XCTestExpectation
    let unexpected: XCTestExpectation
    init(started: XCTestExpectation, unexpected: XCTestExpectation) { self.started = started; self.unexpected = unexpected }
    func endpointTransferType(device: USBDevice, endpoint: UInt8) -> USBTransferType? { .bulk }
    func executeBulkTransfer(device: USBDevice, request: USBRequestBlock) async throws -> USBTransferResult {
        if request.seqnum == 1 { started.fulfill(); await gate.wait() }
        if request.seqnum == 2 { unexpected.fulfill() }
        return USBTransferResult(status: .success, actualLength: 0)
    }
    func executeControlTransfer(device: USBDevice, request: USBRequestBlock) async throws -> USBTransferResult { try await executeBulkTransfer(device: device, request: request) }
    func executeInterruptTransfer(device: USBDevice, request: USBRequestBlock) async throws -> USBTransferResult { try await executeBulkTransfer(device: device, request: request) }
    func executeIsochronousTransfer(device: USBDevice, request: USBRequestBlock) async throws -> USBTransferResult { throw RelayError("Not supported") }
    func openUSBInterface(device: USBDevice, interfaceNumber: UInt8) async throws {}
    func closeUSBInterface(device: USBDevice, interfaceNumber: UInt8) async throws {}
    func isInterfaceOpen(device: USBDevice, interfaceNumber: UInt8) -> Bool { true }
    func validateDeviceClaim(device: USBDevice) throws -> Bool { true }
    func cancelAllTransfers(device: USBDevice, interfaceNumber: UInt8) async throws { await gate.release() }
    func cancelTransfers(device: USBDevice, interfaceNumber: UInt8, endpoint: UInt8) async throws { await gate.release() }
}
final class TransferTests: XCTestCase {
    func request(_ seq: UInt32, command: UInt32 = 1, endpoint: UInt32 = 1, target: UInt32 = 0) -> Data {
        var data = Data(repeating: 0, count: 48)
        for (offset, value) in [(0,command),(4,seq),(8,UInt32(0x10123)),(12,1),(16,endpoint),(20,target)] {
            var value = value.bigEndian
            withUnsafeBytes(of: &value) { data.replaceSubrange(offset..<offset+4, with: $0) }
        }
        return data
    }
    func testQueuedCancellationAndIndependentEndpoints() async throws {
        let started = expectation(description: "First read reached USB")
        let unexpected = expectation(description: "Cancelled queued request never touches USB"); unexpected.isInverted = true
        let other = expectation(description: "Another endpoint completes while the first is blocked")
        let cancelled = expectation(description: "Queued request is cancelled")
        let finished = expectation(description: "First read completes")
        let usb = FixtureUSB(started: started, unexpected: unexpected)
        let transfers = Transfers(usb, device: fixtureDevice().usb, output: { data in
            switch u32(data, 4) {
            case 1: finished.fulfill()
            case 3: other.fulfill()
            case 4: XCTAssertEqual(Int32(bitPattern: u32(data, 20)), -104); cancelled.fulfill()
            default: XCTFail("Unexpected response")
            }
        }, fatal: { XCTFail("\($0)") })
        try transfers.enqueue(request(1))
        await fulfillment(of: [started], timeout: 2)
        try transfers.enqueue(request(2))
        try transfers.enqueue(request(3, endpoint: 2))
        try transfers.enqueue(request(4, command: 2, target: 2))
        await fulfillment(of: [other, cancelled], timeout: 2)
        await usb.gate.release()
        await fulfillment(of: [finished], timeout: 2)
        await fulfillment(of: [unexpected], timeout: 0.1)
    }
}
