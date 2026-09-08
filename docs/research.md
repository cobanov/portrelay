# Research and reuse assessment

Reviewed: 2026-09-08. This is a review of primary project documentation, not a
hardware benchmark or a completed interoperability test. Upstream support claims
below are attributed to those projects. PortRelay has no implementation yet.

## Existing solutions

| Project | What it provides | Reuse decision and limitation |
| --- | --- | --- |
| [Linux USB/IP](https://docs.kernel.org/usb/usbip_protocol.html) and [userspace tools](https://github.com/torvalds/linux/tree/master/tools/usb/usbip) | USB request transport, host export, and a client virtual host controller. | First backend. Keep its wire format and existing drivers. PortRelay must add identity, authorization, encrypted transport, ownership, recovery, and an installer. |
| [usbipd-win](https://github.com/dorssel/usbipd-win) | Windows USB export service, CLI, and installer. | Preferred Windows exporter candidate. Its WSL attach command attaches into WSL; this is not a native Windows import backend. The default installer creates a LAN firewall rule that PortRelay must account for. |
| [usbip-win2](https://github.com/vadimgrn/usbip-win2) | Native Windows USB/IP client using a virtual USB driver; upstream advertises signed drivers. | Preferred Windows importer candidate. Verify the actual installer signature, architecture, driver lifecycle, and loopback bridge before integration. An upstream release is not PortRelay validation. |
| [usbip-win](https://github.com/cezanne/usbip-win) | Windows export and import implementations. | Useful historical reference; compare device behavior before selecting it over the separate exporter/importer above. |
| [usbip-macos](https://github.com/carlossless/usbip-macos) | Experimental macOS USB/IP client. | Feasibility reference. Its README requires a restricted USB host-controller entitlement or a development environment with SIP disabled. The latter is unsuitable for the product. No reusable license was returned by GitHub's license endpoint in this review; do not copy its code without resolving that. |
| [usbipd-mac](https://github.com/beriberikix/usbipd-mac) | Proposed macOS USB/IP server and system extension. | Track separately from macOS import. Its README currently says Apple approval is still required and the project will not work until approved. Installation documentation is not proof that the backend is usable. |
| [usbredir / SPICE](https://www.spice-space.org/api/spice-gtk/SpiceUsbredirChannel.html) | USB redirection into virtual machines. | Useful if VM support is added. It does not by itself provide a desktop OS virtual USB controller on every target platform. |
| [usbip-gui](https://github.com/K-Francis-H/usbip-gui) | A Linux graphical wrapper around USB/IP. | Useful UX reference. Its documented Linux-only scope does not cover the intended cross-platform product. |
| [VirtualHere](https://www.virtualhere.com/) and its [client](https://www.virtualhere.com/usb_client_software) | Commercial USB sharing with device discovery and a simple connect interaction. | Product experience reference, not a runtime dependency. No proprietary binary will be required by PortRelay. |
| [Bumble](https://github.com/google/bumble) and its [transports](https://google.github.io/bumble/transports/index.html) | Bluetooth host stack with Classic and BLE support and network HCI transports. | Candidate for controller experiments and automated Bluetooth tests. Network HCI transport alone does not install a virtual Bluetooth controller into Windows or macOS. |
| [btleplug](https://github.com/deviceplug/btleplug) | Rust BLE central access on Linux, Windows, and macOS. | Candidate for discovery and a later GATT bridge. Upstream explicitly excludes Bluetooth Classic; it is not a general Bluetooth virtualization layer. |
| [ESPHome Bluetooth Proxy](https://esphome.io/components/bluetooth_proxy/) | BLE advertisement and active GATT forwarding for Home Assistant. | Reference for a service-level BLE bridge. Its integration model does not replace arbitrary desktop Bluetooth profiles. |

## What “Bluetooth over a network” actually means

There are three different products hiding under that label:

1. **Loan a whole USB Bluetooth controller.** Carry the adapter through the USB
   backend and let the receiving operating system run its Bluetooth stack. A
   dedicated adapter avoids taking away the owner's keyboard, mouse, or audio.
   Only one computer owns the controller at a time. The radio remains physically
   near the original computer; pairing is performed by the receiving stack.
   VirtualHere's [adapter-sharing example](https://www.virtualhere.com/android)
   illustrates this approach, but PortRelay must validate its own implementation.
2. **Forward controller HCI traffic.** Bumble offers relevant transports, but the
   receiver still needs a host stack or OS integration. Access to built-in
   controllers and their firmware is platform-specific. This remains a research
   track, not an assumed replacement for the USB path.
3. **Proxy individual BLE services.** Use the local OS Bluetooth API and forward
   discovery, reads, writes, and notifications to a compatible consumer. This is
   useful without lending the whole adapter, but it does not preserve every
   profile, device identity, pairing model, or unmodified application's behavior.

**Decision:** Start with a dedicated USB Bluetooth adapter for transparent
controller sharing. Keep a separate milestone for individual BLE devices. Do not
advertise universal built-in Bluetooth, headset, or game-controller support on
the strength of a GATT demonstration.

## Transport and desktop choices

[iroh](https://github.com/n0-computer/iroh) provides a Rust networking foundation
with encrypted QUIC connections, endpoint identities, NAT traversal, and relay
fallback. This addresses a larger part of the internet setup problem than a raw
TCP socket. Its [relay source is open](https://github.com/n0-computer/iroh/tree/main/iroh-relay).
The [production relay guidance](https://docs.iroh.computer/add-a-relay) recommends
dedicated or self-hosted infrastructure; shared public relays are intended for
development and testing. We must not promise production capacity from them.

[Tauri's distribution tooling](https://tauri.app/distribute/) covers native
desktop packaging. Pairing it with a Rust agent allows shared protocol and state
types while keeping the privileged helper small. Tauri packaging does not solve
kernel-driver signing, Apple entitlements, or device compatibility.

**Decision:** Rust agent and helper, Tauri desktop shell, USB/IP platform
backends, and iroh transport. A CLI will exercise the same agent before desktop
work. An Electron or Go implementation remains possible, but neither removes
the backend constraints; Rust fits the selected networking library directly.

## License inventory before implementation

PortRelay's original work uses MIT. No external source or binary has been copied
into the repository. This is a candidate inventory, not a distribution audit:

- Linux backend: kernel and USB/IP tooling keep their upstream, per-file licenses.
  Prefer distribution packages and a process boundary.
- usbipd-win: [GPL-3.0-only](https://github.com/dorssel/usbipd-win).
- usbip-win2: [BSD-2-Clause](https://github.com/vadimgrn/usbip-win2/blob/master/LICENSE.txt).
- Bumble: [Apache-2.0](https://github.com/google/bumble/blob/main/LICENSE).
- btleplug: [BSD-3-Clause with additional inherited notices](https://github.com/deviceplug/btleplug/blob/master/LICENSE.md).
- iroh: [MIT or Apache-2.0](https://github.com/n0-computer/iroh#license).

Before bundling anything, pin an exact revision, inspect all distributed
components, preserve required notices, and satisfy any source-distribution
requirements. A project's top-level license badge is not a complete inventory
of its installer or transitive dependencies.

## Questions the first prototype must answer

- Can the Linux virtual controller and the Windows client attach through an
  access-controlled local bridge without exposing another device?
- Can device detachment, driver restoration, and ownership release recover after
  a killed process, cable removal, sleep, or lost network?
- Which dedicated Bluetooth adapters support reconnect and pairing over this
  path, and which fail under added latency or packet loss?
- Can macOS export or import be shipped with normal security settings and
  obtain the necessary permissions? If not, keep those features unavailable.
- What relay deployment and sustainable bandwidth policy can make the default
  internet experience simple without requiring a paid proprietary service?
