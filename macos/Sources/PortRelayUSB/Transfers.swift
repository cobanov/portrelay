import Foundation
import Common
import USBIPDCore

private actor Admission {
    var ready = false
    var waiters: [CheckedContinuation<Void, Never>] = []
    func wait() async {
        if ready { return }
        await withCheckedContinuation { waiters.append($0) }
    }
    func release() {
        ready = true
        let pending = waiters; waiters.removeAll()
        pending.forEach { $0.resume() }
    }
}
private final class Pending: @unchecked Sendable {
    let isSubmit: Bool
    init(isSubmit: Bool) { self.isSubmit = isSubmit }
    var started = false
    var cancelled = false
    let admission = Admission()
}
final class Transfers: @unchecked Sendable {
    private let submit: USBSubmitProcessor
    private let unlink: USBUnlinkProcessor
    private let output: @Sendable (Data) throws -> Void
    private let fatal: @Sendable (Error) -> Void
    private let writeLock = NSLock()
    private let lock = NSLock()
    private var requests: [UInt32: Pending] = [:]
    private var tails: [UInt32: Task<Void, Never>] = [:]
    init(_ communicator: USBDeviceCommunicator, device: USBDevice,
         output: @escaping @Sendable (Data) throws -> Void = { try FileHandle.standardOutput.write(contentsOf: $0) },
         fatal: @escaping @Sendable (Error) -> Void = { fputs("Mac USB transfer failed: \($0)\n", stderr); exit(1) }) {
        let config = ServerConfig()
        config.maxUSBBufferSize = UInt32(maximumTransfer)
        config.usbOperationTimeout = 15000
        submit = USBSubmitProcessor(deviceCommunicator: communicator, deviceDiscovery: SelectedDiscovery(device), config: config)
        unlink = USBUnlinkProcessor()
        unlink.setSubmitProcessor(submit)
        unlink.setDeviceCommunicator(communicator)
        self.output = output; self.fatal = fatal
    }
    func enqueue(_ request: Data) throws {
        let seq = u32(request, 4), command = u32(request, 0)
        lock.lock(); defer { lock.unlock() }
        // Reserve extra space for cancellation, including when submissions fill the queue.
        guard requests[seq] == nil, requests.count < (command == 2 ? 80 : 64) else {
            throw RelayError("Too many outstanding requests or duplicate sequence")
        }
        let pending = Pending(isSubmit: command == 1)
        requests[seq] = pending
        if command == 1 {
            // Keep each endpoint ordered without letting a pending read block writes
            // on another endpoint. Both directions of endpoint zero share a queue.
            let endpoint = u32(request, 16)
            let lane = endpoint == 0 ? 0 : (u32(request, 12) << 8) | endpoint
            let previous = tails[lane]
            tails[lane] = Task.detached {
                await previous?.value
                if self.begin(pending) {
                    do {
                        let response = try await self.submit.processSubmitRequest(request, registered: {
                            Task { await pending.admission.release() }
                        })
                        try self.write(response)
                    } catch { self.fatal(error) }
                }
                await pending.admission.release()
                self.finish(seq, pending)
            }
        } else {
            let target = requests[u32(request, 20)].flatMap { $0.isSubmit ? $0 : nil }
            let cancelledQueued = target.map { !$0.started && !$0.cancelled } ?? false
            if cancelledQueued { target?.cancelled = true }
            Task.detached {
                do {
                    let response: Data
                    if cancelledQueued {
                        response = try USBIPMessageEncoder.encodeUSBUnlinkResponse(seqnum: seq,
                            devid: u32(request, 8), direction: u32(request, 12), ep: u32(request, 16), status: -104)
                    } else {
                        // A SUBMIT ahead of this UNLINK must enter the core's URB table
                        // before cancellation can look it up. It need not finish I/O.
                        if let target { await target.admission.wait() }
                        response = try await self.unlink.processUnlinkRequest(request)
                    }
                    try self.write(response)
                } catch { self.fatal(error) }
                self.finish(seq, pending)
            }
        }
    }
    private func begin(_ pending: Pending) -> Bool {
        lock.lock(); defer { lock.unlock() }
        pending.started = true
        return !pending.cancelled
    }
    private func write(_ data: Data) throws {
        writeLock.lock(); defer { writeLock.unlock() }
        if !data.isEmpty { try output(data) }
    }
    private func finish(_ seq: UInt32, _ pending: Pending) {
        lock.lock(); defer { lock.unlock() }
        if requests[seq] === pending { requests.removeValue(forKey: seq) }
    }
}
