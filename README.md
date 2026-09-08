# PortRelay

**Your devices, on either computer.**

Open-source USB sharing between Windows and Linux computers, with an encrypted connection
and a small local control window. No account or subscription is required.

> **Windows + Linux developer alpha.** Native USB serial traffic has passed
> Windows-to-Linux and Linux-to-Windows tests using isolated virtual devices.
> Windows 11 x64 uses signed upstream drivers with Secure Boot enabled.
> Physical USB and Bluetooth compatibility still need testing. macOS USB
> support is not implemented. Read the [validation record](docs/validation.md).

**[Website](https://portrelay.cobanov.dev)** ·
**[Download the alpha](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.3)** ·
**[Windows setup](docs/windows-alpha.md)** · **[Linux setup](docs/linux-alpha.md)**

## Install on both computers

1. Download and open the **Windows**, **Ubuntu**, or **Debian** installer on
   each computer.
2. Open **PortRelay** from your applications. Choose **Enable USB sharing**,
   then add your other computer with an invitation and approve it.
3. Choose **Share device** on one computer and **Connect** on the other.
   **Disconnect** returns the device to its owner.

The package installs dependencies, adds the app shortcut, and keeps the agent
running after you close its window. An administrator prompt enables USB support;
you do not need to configure helper services manually. See the
[short setup guide](docs/linux-alpha.md) for downloads and system requirements.

The local interface shows actual devices, paired computers, permissions,
connections, and recovery errors. Each device is private until explicitly
shared. One computer can use a shared device at a time. Removing trust or
stopping sharing ends an active loan.

LAN mode works without a public service. Internet routes use an optional
configurable iroh relay, including a relay-only mode for blocked UDP networks.
A forced external relay path has been tested; PortRelay does not run a shared
production relay. See [network setup](docs/linux-alpha.md#networks).

## USB and Bluetooth boundaries

Linux USB export and import use the existing USB/IP kernel drivers. Windows
uses a separate modified usbipd-win service and the signed usbip-win2 native
virtual controller, without WSL. See [Windows architecture](docs/adr-0003-windows-alpha.md).

On Linux, a separate
root helper binds devices, supplies private sockets to the kernel, and restores
the original driver. The application never starts a raw USB/IP network server.

A dedicated **USB Bluetooth adapter** can use the same whole-device path. Its
radio stays near the original computer; the receiving OS would own the adapter.
The UI requires acknowledgement before sharing it. This path has not yet passed
physical Bluetooth pairing or peripheral tests. Individual BLE services,
Bluetooth audio/profile forwarding, and arbitrary built-in controller migration
are not implemented.

Storage, input devices, hubs, imported devices, and detected network adapters
are blocked in this alpha. Printers, cameras, and other physical devices also
need their own validation. The website's printer, Bluetooth, and drive examples
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
macOS can build the control agent, but it cannot export or attach USB devices.

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
