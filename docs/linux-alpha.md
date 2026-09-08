# Linux developer alpha

PortRelay now contains a real Rust application, a local browser control window,
and a separate Linux USB/IP helper. This is an experimental developer alpha.
It is not a claim of general USB or Bluetooth device compatibility.

## Install on both computers

The downloadable package targets x86_64 Linux (glibc 2.36 or newer), with systemd
and the distribution's USB/IP tools and kernel modules. ARM64 is buildable in
principle but has no validated package yet.
Ubuntu needs `linux-tools-common` and the tools matching the running kernel;
Debian provides the `usbip` package. `usbip-host` and `vhci-hcd` must be available.
A desktop browser and `xdg-utils` open the control window.

Download the tarball and checksum from the
[alpha release](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.1).
On each computer, open a terminal in the download folder:

```sh
sha256sum -c portrelay-0.1.0-alpha.1-linux-x86_64.tar.gz.sha256
tar -xzf portrelay-0.1.0-alpha.1-linux-x86_64.tar.gz
cd portrelay-0.1.0-alpha.1-linux-x86_64
sudo ./install-linux.sh "$USER"
portrelay
```

The checksum detects download corruption; it is not a publisher signature.

Build from the repository with Rust 1.97.0:

```sh
cargo build --locked --release
sudo ./packaging/install-linux.sh "$USER" target/release/portrelay
portrelay
```

The installer starts an unprivileged application service for the selected user
and a root helper. It adds an applications-menu shortcut. One local user is
supported per installation. The control page binds only to `127.0.0.1`, uses a
random access token, and contains live device state. It is separate from the
simulated public website.

The CI `portrelay-linux-x86_64-alpha` artifact contains a Linux tarball, its
SHA-256 checksum, and install/uninstall scripts. These are developer artifacts,
not a signed stable release. They expire after 30 days; use the release assets
for the published alpha. Extract the tarball, verify its checksum, and run
`sudo ./install-linux.sh "$USER"` from the extracted directory.

## Share one device

1. On the device-owning computer, choose **Pair a computer**. Send its invitation
   privately to the other computer and paste it there. Approve the pending
   computer on the owner. Compare the displayed endpoint identities if needed.
2. Select that paired computer, then choose **Share** beside the USB device.
   Sharing is a per-device, per-computer permission. The original computer keeps
   the device until the remote computer connects.
3. On the receiving computer, choose **Other computer**, then **Connect**.
   Its operating system attaches the virtual USB device. Normal applications
   can use it if their driver supports that particular device and network path.

Use **Disconnect** to end a loan. **Stop sharing** also closes active loans.
Removing a computer revokes its grants and closes its active sessions.
Unplugging/replacing a device invalidates its old permission; share it again.

Input devices, storage, hubs, imported devices, and detected network adapters
are deliberately unavailable in this alpha. Start with a disposable USB serial
fixture. Printers and dedicated USB Bluetooth adapters need their own physical
compatibility results before they should be relied on.

For Bluetooth, the whole USB adapter is lent to one computer. Its radio remains
near the owner. Existing local Bluetooth connections may stop. The UI requires
an explicit acknowledgement before granting adapter access. Individual BLE
services, built-in controller migration, and universal audio/gamepad support
are not implemented.

## Networks

The default agent listens for authenticated QUIC on UDP 24816. It uses no public
relay or address-publishing service. Allow that UDP port between the intended
computers if a firewall blocks it. Raw USB/IP port 3240 is never opened by
PortRelay. A valid invitation and owner approval are still required on a LAN.

On different networks, configure an iroh-compatible relay that you operate or
are authorized to use:

```sh
portrelay run --relay https://your-relay.example
```

The same pairing and device permissions apply. `--relay-only` disables direct
IP transports and forces the configured relay, useful for diagnosing networks
that block UDP. Public upstream relays are for development/testing; PortRelay
does not provide a production relay or promise free hosted bandwidth. The
relay sees routing metadata but carries an end-to-end encrypted device stream.

Stop the installed `portrelay-agent` service before running a custom command
with the same data directory. For a persistent custom relay, configure a systemd
service override for `ExecStart`, then restart the agent when no devices are in
use. Address changes may require a fresh invitation in this alpha; automatic
LAN discovery and address refresh are not implemented yet.

## Troubleshooting and removal

- `portrelay status` reports devices, peers, helper health, and active sessions.
- `portrelay check` exits successfully only when both agent and helper are ready.
- `portrelay open` reopens the authenticated local window.
- `journalctl -u portrelay-helper` reports device restoration errors.
- A helper recovery error disables further attachments and appears in the UI.
  Restart `portrelay-helper` to retry recovery, then inspect the actual device.
- Closing a browser tab leaves the agent and its device sessions running.
  Disconnect in the app before shutting it down or upgrading.
- USB connection state means the transport and virtual controller are attached;
  application-level readiness still depends on OS enumeration and device drivers.
- Stop active sharing and the agent service before reinstalling/upgrading.
- `sudo ./packaging/uninstall-linux.sh` removes the services and binary after
  successful device cleanup. It preserves the user's identity and pairing data.

No SIP, Secure Boot, or signature enforcement changes are required. macOS can
build the control application but cannot attach/export USB with this backend.
Windows device integration is not implemented.
