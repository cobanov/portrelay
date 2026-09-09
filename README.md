# PortRelay

**Your devices, on either computer.**

Open-source USB sharing between Windows and Linux computers, with an encrypted connection
and a small local control window. Mac users can also send keyboard and mouse
control to an unlocked Linux desktop. Sign in with GitHub to find your computers automatically, or pair offline. No subscription is required.

> **v0.1.0-alpha.8: GitHub sign-in and automatic computer registration.** Native USB serial traffic has passed
> Windows-to-Linux and Linux-to-Windows tests using isolated virtual devices.
> Windows 11 x64 uses signed upstream drivers with Secure Boot enabled.
> Physical USB and Bluetooth compatibility still need testing. A separate [Mac export development preview](docs/macos-alpha.md)
> is available as a signed, notarized Apple Silicon app. Mac USB receiving is not implemented;
> physical Mac export still needs testing. Read the [validation record](docs/validation.md).
> Windows has a known driver-removal stall after forced app shutdown; start on a
> test computer and read the [Windows limitations](docs/windows-alpha.md).

**[Website](https://portrelay.cobanov.dev)** ·
**[Download the alpha](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.8)** ·
**[Mac preview](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-macos-preview.3)** ·
**[Windows setup](docs/windows-alpha.md)** · **[Linux setup](docs/linux-alpha.md)** · **[Raspberry Pi / headless](docs/raspberry-pi.md)**

## Your computers, one account

Choose **Sign in with GitHub** on each computer. They appear automatically.
For SSH-only computers, run `portrelay login --no-open` and open the link on your
laptop. [Account setup, removal and privacy](docs/accounts.md).

## Use your Mac mouse on Linux

1. Install **Mac preview.3** and **Linux alpha.8**, then sign in with the same GitHub account.
2. On Linux, choose **Enable receiving control**, then **Allow keyboard & mouse**
   for your Mac.
3. On Mac, choose **Control [computer]**. **Esc** returns control to the Mac.

Keep the Linux screen visible. This feature sends input only, with no video or
clipboard. It works independently of USB/IP and does not detach the Mac's mouse.
[Setup and current limits](docs/input-control.md).

## Install on both computers

1. Download and open the **Windows**, **Ubuntu**, or **Debian** installer on
   each computer.
2. Open **PortRelay** from your applications. Choose **Enable USB sharing**,
   then sign in with the same GitHub account on both computers.
3. Choose **Share device** on one computer and **Connect** on the other.
   **Disconnect** returns the device to its owner.

Prefer a terminal? Install the same alpha package with one command:

**Ubuntu 24.04 / Debian 12–13 / Raspberry Pi OS (AMD64, ARM64, ARMv7):**

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh | sh
```

**Raspberry Pi OS Lite / SSH-only Linux:**

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh | sh -s -- --headless
portrelay login --no-open
portrelay menu
```

This explicitly enables USB setup and starts the regular user's agent at boot,
without a desktop or login. Devices stay private until shared. ARMv7 requires
Pi 2 or newer, including Zero 2 W; Pi 1/original Zero ARMv6 are excluded.
Physical Pi USB/Bluetooth compatibility is still unvalidated.
[Pi requirements](docs/raspberry-pi.md) · [Terminal menu and short commands](docs/terminal.md).

**Windows 11 (Intel/AMD 64-bit), in PowerShell:**

```powershell
irm https://portrelay.cobanov.dev/install.ps1 | iex
```

The scripts verify the pinned release SHA-256 before installing. Linux selects
its distribution package and installs dependencies; Windows opens the existing
installer with the normal administrator prompt. Then open PortRelay to enable
USB and pair computers. [Read the Linux script](web/install.sh) or
[Windows script](web/install.ps1). These currently install alpha.6, without input control or auto-updates. Use the
alpha.7 release packages for keyboard and mouse control until the scripts are updated.

The package installs dependencies, adds the app shortcut, and keeps the agent
running after you close its window. An administrator prompt enables USB support;
you do not need to configure helper services manually. Follow the
[Windows guide](docs/windows-alpha.md) or [Linux guide](docs/linux-alpha.md)
for system requirements and troubleshooting.

The local interface shows actual devices, paired computers, permissions,
connections, and recovery errors. Each device is private until explicitly
shared. One computer can use a shared device at a time. Removing trust or
stopping sharing ends an active loan.

LAN mode works without a public service. Internet routes use an optional
configurable iroh relay, including a relay-only mode for blocked UDP networks.
A forced external relay path has been tested; PortRelay does not run a shared
production relay. See [network setup](docs/linux-alpha.md#networks).

**Mac preview:** [download the ZIP](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-macos-preview.3), unzip, move PortRelay to Applications, then open it.
Apple Silicon / macOS 14+. Control a Linux desktop with your keyboard and mouse,
or try limited USB export. Mac USB receiving and Bluetooth are unavailable. [Mac setup and device scope](docs/macos-alpha.md).

## USB and Bluetooth boundaries

Linux USB export and import use the existing USB/IP kernel drivers. Windows
uses a separate modified usbipd-win service and the signed usbip-win2 native
virtual controller, without WSL. See [Windows architecture](docs/adr-0003-windows-alpha.md).

On Linux, a separate
root helper binds devices, supplies private sockets to the kernel, and restores
the original driver. The application never starts a raw USB/IP network server.

A dedicated **USB Bluetooth adapter** can use the same whole-device path. Its
radio stays near the original computer; the receiving OS would own the adapter.
The UI requires acknowledgement before sharing it and opens the receiving OS's
Bluetooth settings after connection. This path has not yet passed
physical Bluetooth pairing or peripheral tests. Individual BLE services,
Bluetooth audio/profile forwarding, and arbitrary built-in controller migration
are not implemented.

Alpha.4 enables keyboards/mice, unmounted Linux disks, offline Windows disks,
and disabled USB network adapters with explicit handoff warnings. Hub groups
share the currently connected devices; the hub itself stays local. Source usage
is checked again before export. Imported devices cannot be re-exported.

Linux kernel tests passed disk write/readback, keyboard/mouse events and USB
Ethernet traffic. These used virtual USB fixtures, not physical devices. Windows
has the same conditional policy, but these new classes still need native Windows
handoff tests. See [device sharing and Bluetooth setup](docs/device-sharing.md).
Printers, cameras, and other physical devices also need their own validation. The website's printer, Bluetooth, and drive examples
remain explicitly simulated product concepts.

## Build the application

Use Rust 1.97.0. Dependencies are pinned in `Cargo.lock`.

```sh
cargo build --locked --release
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

On Linux, install the privileged helper and unprivileged application service:

```sh
sudo ./packaging/install-linux.sh "$USER" target/release/portrelay
portrelay
```

The [installation guide](docs/linux-alpha.md) covers prerequisites, downloads,
headless operation, internet settings, diagnostics, upgrades, and uninstalling.
Build the Windows installer with `./windows/build.ps1` in PowerShell on a
Windows x64 build machine with Rust 1.97, .NET SDK 8 and 9.0.317, Python 3, Git,
and Inno Setup 6. See [Windows source notices](windows/THIRD-PARTY-NOTICES.md).
For the limited native Mac exporter and app bundle, see [Mac setup](docs/macos-alpha.md). Mac receiving support remains unavailable.

The implementation uses Rust/Tokio, iroh 1.1.0 QUIC, an Axum loopback API, and
embedded HTML/CSS/JavaScript. A native Tauri shell is deferred. See
[ADR 0002](docs/adr-0002-linux-alpha.md) for the implementation decision.

## Website development

The separate public demo in `web/` has no access to the installed agent or
hardware. With Node.js 22 or newer:

```sh
npm install
npm run dev
npm run check
npm run build
```

Cloudflare Pages serves the production site. `npm run deploy` uploads the clean
committed `dist/` build using the existing Cloudflare credentials and pinned
Wrangler. See [website notes](docs/website.md).

## Project documents

- [Research and reuse assessment](docs/research.md)
- [Initial architecture](docs/architecture.md) and [implemented alpha](docs/adr-0002-linux-alpha.md)
- [Validation and known limitations](docs/validation.md)
- [Implemented security boundaries](docs/security-model.md)
- [Roadmap](docs/roadmap.md), [contributing](CONTRIBUTING.md), and [security policy](SECURITY.md)

## License

PortRelay's Rust agent, local UI, and website are [MIT licensed](LICENSE). Its
separate Windows device service is GPL-3.0-only and ships with corresponding
source. Upstream components retain their own licenses. Packages include
`Cargo.lock`, a CycloneDX SBOM, and `THIRD_PARTY_NOTICES.txt`.
Distribution USB/IP tools and kernel drivers retain their own licenses; the
Debian/Ubuntu setup installs them through the distribution package manager.
