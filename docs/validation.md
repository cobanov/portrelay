# Alpha validation record

Date: 2026-09-08. Version: `0.1.0-alpha.1`.

## What works in the recorded setup

An unprivileged PortRelay agent on two Linux computers, each with the restricted
root helper, carries a USB serial device through the native USB/IP and virtual
host controller drivers. A normal process opens the resulting `/dev/ttyACM*`
and exchanges bytes. This is kernel device traffic, not a mocked byte stream.

The source device was an explicitly created **virtual** ACM serial gadget using
Linux `dummy_hcd` and configfs, VID:PID `1d6b:0104`. It was created in a disposable
Debian VM, not bound from physical hardware. Production export code uses the
same `usbip-host` backend and distribution USB/IP executable.

- Exporter: Debian 13, x86_64, kernel `6.12.107+deb13-amd64`, distribution `usbip`.
- Importer: Ubuntu 24.04.4, x86_64, kernel `7.0.0-30-generic`, `vhci-hcd`.
- Transfer: 65,536 bytes, sent and echoed in 512-byte blocks.
- SHA-256: `7daca2095d0438260fa849183dfc67faa459fdf4936e1bc91eec6b281b27e4c2`.
- Direct QUIC: exact payload match, disconnect, reconnect, and owner revocation.
- Repetition: 20 additional direct connect/disconnect cycles passed.
- Forced relay: exact payload match through an external upstream development
  iroh relay, with **all direct IP transports disabled on both agents**.
  Three additional relayed connect/disconnect cycles and owner revocation passed.
- Each clean disconnect checked both agent session lists, imported-device
  removal, empty helper recovery journals, healthy helpers, and restoration of
  the source device's `usb` driver. No lingering USB/IP stub workers remained.
- Abrupt helper termination: `SIGKILL` on the disposable exporter's helper,
  systemd restart, journal reconciliation, original driver restoration, and a
  subsequent new connection passed over the relay.
- Abrupt importing-agent termination: `SIGKILL` closed the imported device;
  the exporter restored its original driver and both recovery journals cleared.

The direct fixture used an explicit UDP forward into the disposable VM. The
relay test used an actual external internet relay, but the endpoints shared an
upstream connection. This does not prove direct hole punching across two
independent NATs, geographic latency, or production relay availability.

## Installation evidence

The binary was built for x86_64 Linux in the Rust 1.97 Debian Bookworm image
(glibc 2.36 baseline). The binary package was installed on a fresh Debian 13
cloud VM without Rust or a JavaScript toolchain. Agent/helper systemd services
started successfully, including after a guest reboot. The local agent reported
real helper health; a different unprivileged user was denied access to its Unix
socket. Uninstall removed the services and executable while preserving identity
and pairing files. The final installer also waits for API and helper readiness.
The package includes a checksum, license notices, Cargo lockfile, and CycloneDX
SBOM. This headless test does not validate a graphical applications-menu launch.

## Automated checks

Linux has 11 Rust tests covering protocol size/path validation, invitation
expiry and single use, real QUIC pairing and approval, authorization and exclusive
leases, local API authentication/origins, private TCP socket construction,
virtual controller status parsing, encrypted streams, cancellation, and backend
failure propagation. Seven portable tests also pass on macOS. Linux-only tests
are explicitly gated; a macOS test pass does not imply a USB backend exists.

Formatting, Clippy with warnings denied, local UI JavaScript syntax, website
syntax/build checks, and the packaged binary are checked separately. The test
suite's in-memory streams are distinct from the kernel tests above. Browser
rendering, real desktop-menu launch, and usability with a non-technical person
have not been observed in this session.

`cargo audit` against the advisory database on this date found **zero known
vulnerabilities** and one unmaintained-dependency warning: `paste 1.0.15`,
[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html), transitively
used by Linux network discovery dependencies in iroh. It is a build-time macro;
the warning remains tracked rather than suppressed. This is not an independent
security audit. Packages carry dependency versions and upstream notices.

## Corrections discovered during kernel testing

- A fast disconnect could race `usbip unbind` against the kernel's own session
  shutdown, leaving restoration incomplete. The helper now waits for the kernel
  export state before unbinding, verifies restoration, and retains failed
  recovery records. The agent waits for a helper acknowledgement before ending
  the lease. The repeated tests above use this corrected path.
- Import EOF can empty a virtual port just before the explicit detach write.
  Cleanup accepts that case only after verifying that the port is already empty.
- A fresh headless VM may not have `/sys/bus/usb` until modules load. The service
  sandbox now uses the existing `/sys/bus` parent so the helper can start at boot.

An earlier experimental `usbip-vudc` fixture on a Linux 7.0 source host completed
one byte transfer, after which that host became unreachable. Its root cause has
not been established and its recovery was not verified. That fixture/backend
has been removed from the application and test scripts. Subsequent kernel work
used a disposable Debian VM and the production `usbip-host` path. This incident
is an unresolved limitation, not a successful reliability result.

## Still unvalidated or unavailable

- Physical USB serial, printers, dedicated Bluetooth adapters, and peripheral
  pairing. No physical-device support matrix is claimed.
- Bluetooth peripheral use, BLE profile forwarding, audio, gamepads, and built-in
  controller migration. Whole USB-adapter sharing code is not Bluetooth proof.
- Storage/input/hub/network sharing: blocked in the alpha. Audio/video-only
  classes are not enabled; other physical device classes are also unvalidated.
- Windows export/import, macOS export/import, and packaged ARM64 builds.
- Physical hotplug, host suspend/resume, sustained workloads, packet-loss tests,
  power loss, and additional kernel recovery behavior.
- Automatic discovery/reconnect/address updates, a hosted production relay,
  signed installers, auto-updates, and a zero-terminal installation experience.

## Reproduce safely

Use a disposable Linux VM for the exporter. `tests/kernel-gadget.sh start`
creates only the named virtual fixture; `stop` removes it. The guest requires
configfs, `dummy_hcd`, distribution USB/IP tools, and the PortRelay helper.
Never use a production input, network, or storage device as the fixture.

`tests/two-host-smoke.py --help` documents the two SSH hosts, agent state path,
and optional VM UDP forward. It exercises transfer, disconnect, repeated
connections, and trust revocation. Its defaults use `/run/portrelay` on the
exporter and `/run/portrelay-test` on the importer. Invites/API credentials stay
in process memory and are not printed.

`tests/lifecycle-smoke.py --help` requires explicit test-host confirmation and
service names. It deliberately kills the named helper and agent processes.
Run it only after configuring relay-only agents in the isolated fixture setup.
