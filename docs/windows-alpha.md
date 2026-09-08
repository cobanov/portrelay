# Windows alpha

For **Windows 11 on an Intel or AMD 64-bit computer**. ARM64 Windows, Windows 10,
and Windows Server are not validated by this package. The other computer can
run the Windows alpha or the [Linux alpha](linux-alpha.md); see the exact tested
directions in [validation](validation.md).

## One-command install

Open **PowerShell** as your normal Windows user and paste:

```powershell
irm https://portrelay.cobanov.dev/install.ps1 | iex
```

This downloads the alpha.4 installer, verifies its pinned SHA-256, and opens the
same installation wizard as the download button. Approve the normal Windows
administrator prompt and finish the wizard. Then open **PortRelay** and follow
steps 2 and 3 below. The script does not enable USB or share devices for you.

[Read the script](../web/install.ps1). Windows security settings and execution
policy stay unchanged; if your organization blocks downloaded scripts, use the
`.exe` download instead. Temporary downloads are removed on completion or error.
Re-running opens the same pinned installer; this does not add automatic updates.

## Install and connect

1. Download the Windows `.exe` installer from the
   [release page](https://github.com/cobanov/portrelay/releases) and open it.
2. Open **PortRelay** from Start. Choose **Enable USB sharing** and approve
   the Windows administrator prompt. Then choose **Add computer** on each end.
3. Copy the invitation to the other computer and approve its request. Choose
   **Share device** on the device's computer, then **Connect** on the other one.

**Disconnect** returns the device. Closing the browser window keeps PortRelay
running; it starts again when you sign in to Windows. Settings, trusted computers,
and device permissions stay in your private Windows profile.

The installer includes the app, device service, and signed upstream drivers.
You do not need Rust, .NET, WSL, separate driver downloads, or manual services.
The first USB setup may ask for a restart. Secure Boot and driver-signature
enforcement remain enabled. PortRelay's alpha installer itself is unsigned;
its checksums are on the release page.

**Known alpha limitation:** after repeated imports and a forced app shutdown,
the upstream virtual USB controller stalled during uninstall and Windows shutdown
in a test VM. Resetting that guest and retrying completed removal. PortRelay now
bounds the uninstall wait and retains removal progress, but the underlying driver
stall is unresolved. Start on disposable test computers.

## Devices and networks

The app shows real local and shared devices. A device stays private until its
owner shares it with an approved computer, and only one computer can borrow it.
Removing a computer or stopping sharing also ends its active connections.

USB serial traffic has been exercised with virtual kernel/device fixtures.
This is an experimental alpha, with physical USB and Bluetooth compatibility
still pending. Alpha.4 permits input devices with consent, disks verified offline
and non-system by Windows, and network adapters verified administratively down.
Some removable flash drives cannot be taken offline and remain blocked. Unknown
usage states and imported devices stay blocked. Hub groups share their current
children, not hub hardware. These newly enabled classes still require actual
Windows handoff validation; their Linux fixture results are not Windows evidence.
See [device handoff and Bluetooth pairing](device-sharing.md).

A dedicated USB Bluetooth adapter uses the whole USB-device path. Its radio
stays beside its original computer. Individual Bluetooth peripherals, BLE
services, Bluetooth audio, and built-in controller migration are separate
capabilities and are not provided by this alpha.

Start with both computers on the same reachable network. The app authorizes
and encrypts connections; raw USB/IP is never opened to the network. Automatic
discovery, automatic reconnect, and managed internet connectivity are still
pending. Advanced users can configure a custom iroh relay using the same
`run --relay URL` / `--relay-only` options described in the
[network guide](linux-alpha.md#networks). A hosted PortRelay relay is not included.

## Update or remove

Run the newer PortRelay installer to update. It stops this installation's agent
and waits for devices to return before replacing files. Your identity and
computer list remain in your profile.

Use **Settings → Apps → Installed apps → PortRelay → Uninstall** to remove it.
The uninstaller releases devices and removes the helper, its firewall rule, and
the drivers installed by PortRelay. User settings are retained. If Windows asks
for a restart, restart before reinstalling or trying another USB application.

PortRelay needs a dedicated USB service/controller. It refuses to replace an
existing usbipd-win, VirtualBox USB monitor, or separately installed USBip client.

## If a connection fails

- **Setup needed:** choose Enable USB sharing and approve administrator access.
  If installation requests a restart, restart and open PortRelay again.
- **No shared devices:** approve the other computer, select it, and share a
  device on its original computer. Both PortRelay apps must be running.
- **Device changed:** refresh and share it again. Permissions do not silently
  transfer to a different device that takes the same USB port. Devices without
  a unique hardware identity also require a new share after returning them.
- **Device needs recovery:** disconnect it, restart Windows, and reopen PortRelay.
  Unresolved device recovery blocks new connections and uninstall until it is
  reconciled. Do not manually delete recovery records to bypass that check.

Advanced diagnostics are in **Settings & status → Connection details**. The
privileged setup log is `%ProgramData%\PortRelay\setup.log`; the service records
driver errors in the Windows Application event log under `usbipd-win`.
Do not post invitations, `api.json`, or private identity files in an issue.

The [Windows architecture decision](adr-0003-windows-alpha.md),
[upstream notices](../windows/THIRD-PARTY-NOTICES.md), and
[validation record](validation.md) describe the implementation and its limits.
