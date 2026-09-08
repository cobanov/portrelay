# PortRelay

**Your devices, on either computer.**

Open-source USB sharing between Linux computers, with an encrypted connection
and a small local control window. No account or subscription is required.

> **Linux developer alpha.** The application now transfers real USB/IP traffic
> through Linux kernel drivers. An isolated virtual USB serial device has passed
> two-computer transfer and recovery tests. Physical USB devices and Bluetooth
> adapters still need compatibility testing. Windows and macOS USB backends are
> not implemented. Read the [validation record](docs/validation.md).

**[Website](https://portrelay.cobanov.dev)** ·
**[Download Linux alpha](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.1)** ·
**[Installation guide](docs/linux-alpha.md)**

## Try it on two Linux computers

1. Install the distribution's USB/IP tools, then the PortRelay alpha package.
2. Open **PortRelay** from the applications menu. Exchange an invitation and
   approve the other computer.
3. Choose **Share** beside a local device. On the other computer, select it and
   choose **Connect**. **Disconnect** returns it to its owner.

The local interface shows actual devices, paired computers, permissions,
connections, and recovery errors. Each device is private until explicitly
shared. One computer can use a shared device at a time. Removing trust or
stopping sharing ends an active loan.

LAN mode works without a public service. Internet routes use an optional
configurable iroh relay, including a relay-only mode for blocked UDP networks.
A forced external relay path has been tested; PortRelay does not run a shared
production relay. See [network setup](docs/linux-alpha.md#networks).

## USB and Bluetooth boundaries

Linux USB export and import use the existing USB/IP kernel drivers. A separate
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

PortRelay's original work is [MIT licensed](LICENSE). Upstream components retain
their own licenses. Packages include `Cargo.lock`, a CycloneDX SBOM, and `THIRD_PARTY_NOTICES.txt`.
Distribution USB/IP tools and kernel drivers are installed separately.
