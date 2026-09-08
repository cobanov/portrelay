# PortRelay

**Your devices, on either computer.**

PortRelay is an open-source project to make USB and Bluetooth devices available
to another computer over a local network or the internet, without a subscription
or a required cloud account.

> **Status: website and interactive desktop concept.** The product website
> includes simulated device sharing. There is no native desktop app,
> device forwarding implementation, or installer yet. The capabilities below
> describe the intended product, not released functionality.

## Website and desktop preview

**[Visit portrelay.cobanov.dev](https://portrelay.cobanov.dev).**

The website in `web/` shows sharing through a printer, a Bluetooth adapter, and
a USB drive. Try connecting and disconnecting example remote devices, switching
between paired computers, and sharing local example devices.
The Linux, Windows, and macOS selector shows their actual development status.
All device state stays in browser memory and resets on reload. No hardware API,
local agent, or real device connection is used.

The website uses plain HTML, CSS, and JavaScript, with no runtime packages.
With Node.js 22 or newer:

```sh
npm install
npm run dev
```

Open the local address printed by the server. For a static production build:

```sh
npm run check
npm run build
```

Production is Cloudflare Pages, configured in `wrangler.json`. After building,
`npm run deploy` uploads `dist/` to the `portrelay` project using existing
Cloudflare credentials. It uses pinned Wrangler 4.105.0. Any static host can
serve the same output. `.openai/hosting.json` preserves the earlier private
Sites preview; production no longer depends on it. Source and design decisions
are documented in [the website notes](docs/website.md).

## The intended experience

1. Install PortRelay on both computers.
2. Find the other computer on the local network, or exchange a pairing invitation
   for an internet connection.
3. Approve the connection and choose which devices to share.
4. Select a remote device and click **Connect**. Click **Disconnect** to return it.

One application can share local devices and use remote devices. The device owner
can stop sharing or revoke a paired computer at any time. Devices are private
until explicitly shared.

## USB and Bluetooth

**USB:** The first implementation will reuse existing USB/IP drivers so that
supported remote hardware can appear to normal applications as a local USB
device. Linux is the first end-to-end target, followed by Windows. macOS is a
separate feasibility track because of driver and entitlement constraints.

**Bluetooth:** The first path will share a dedicated USB Bluetooth adapter. The
remote computer will own that adapter and pair with Bluetooth devices near it.
This lends the entire adapter to one computer; it does not independently forward
each device already paired to the owner's built-in Bluetooth controller.

A later BLE service bridge will target individual Bluetooth Low Energy devices.
It will need compatible applications or explicit profile adapters; GATT access
alone does not make a remote device appear in every application's Bluetooth list.
Bluetooth Classic audio, input, and arbitrary built-in controllers require
separate validation.

These boundaries come from the upstream research, not hardware testing of
PortRelay. See the [research](docs/research.md) and
[planned platform matrix](docs/architecture.md#platform-plan).

## Initial technical direction

- **Rust** for the device agent, session state, and privileged helper.
- **Tauri 2** for an installable desktop interface, with a separate headless agent.
- **USB/IP** backends for device export and virtual USB attachment.
- **iroh / QUIC** for authenticated, encrypted peer connections, NAT traversal,
  and an optional self-hostable relay.
- **Local discovery and explicit pairing**, with no account required for LAN use.

These choices are recorded in the [architecture decision](docs/architecture.md).
Versions and dependencies will be pinned when the feasibility work starts.
No upstream code or drivers have been vendored into this repository.

Free software does not imply unlimited free hosted bandwidth. LAN operation must
work without internet services. Internet connections will prefer a direct route;
relay hosting, capacity, and deployment remain implementation work.

## Project documents

- [Research and reuse assessment](docs/research.md)
- [Architecture and platform boundaries](docs/architecture.md)
- [Roadmap and release acceptance criteria](docs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

PortRelay's original work is licensed under the [MIT License](LICENSE).
Upstream drivers and libraries retain their own licenses and attribution.
