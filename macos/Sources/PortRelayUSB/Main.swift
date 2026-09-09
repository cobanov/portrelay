import Foundation
import Common
import USBIPDCore

@main struct MacUSB {
    static func main() async {
        signal(SIGPIPE, SIG_IGN)
        var ready = false
        do {
            let arguments = Array(CommandLine.arguments.dropFirst())
            switch arguments.first {
            case "probe":
                struct Probe: Encodable { let protocolVersion = 1; let usbExport = true; let usbImport = false }
                try frame(Probe())
            case "inventory":
                let boot = try bootID()
                try frame(try inventory().map { $0.relay(boot: boot) })
            case "export":
                guard arguments.count == 3 else { throw RelayError("Expected a selected device and generation") }
                let boot = try bootID()
                guard let current = try inventory().first(where: { $0.id == arguments[1] }) else { throw RelayError("Device was unplugged") }
                let device = current.relay(boot: boot)
                guard device.generation == arguments[2] else { throw RelayError("Device changed; select it again") }
                if let reason = device.blocked { throw RelayError(reason) }
                let claims = UserspaceDeviceClaimManager()
                _ = try claims.claimDevice(current.usb)
                let communicator = USBDeviceCommunicatorImplementation(deviceClaimManager: claims)
                // Opening the exact interface supplies the actual OS ownership check.
                // No driver is detached, no configuration is persisted, and no root helper is used.
                try await communicator.openUSBInterface(device: current.usb, interfaceNumber: 0)
                try frame(Ready(error: nil, device: device))
                ready = true
                let transfers = Transfers(communicator, device: current.usb)
                while let header = try readExactly(48) {
                    let length = try bodyLength(header, expectedDevice: device.devid)
                    let body: Data
                    if length > 0 {
                        guard let value = try readExactly(length) else { throw RelayError("Truncated transfer payload") }
                        body = value
                    } else { body = Data() }
                    try transfers.enqueue(header + body)
                }
                // Process exit closes every IOKit handle, including pending requests.
                // A dead parent closes stdin, so a killed agent cannot leave a loan behind.
            default: throw RelayError("Use probe, inventory, or export DEVICE GENERATION")
            }
            exit(0)
        } catch {
            if !ready { try? frame(Ready(error: String(describing: error), device: nil)) }
            fputs("Mac USB worker: \(error)\n", stderr)
            exit(1)
        }
    }
}
