# macOS export development preview

Mac support has started. This is a **source-build development preview**, separate
from the published Windows/Linux alpha. There is no notarized Mac download yet.

## What is implemented

- The same local app window and terminal menu, encrypted pairing, per-device
  sharing permissions, exclusive sessions, and owner disconnect.
- Native, read-only USB inventory. Devices that cannot be shared show a reason.
- A per-device IOKit export worker connected to the agent through inherited pipes.
  It uses the existing encrypted connection to a Linux or Windows receiver.
- A Mac app bundle and a user LaunchAgent. Opening the installed app starts the
  background agent and opens its authenticated local window. No root setup is
  needed for this limited exporter.

The first exporter only admits **one interface, numbered 0, with vendor-specific
or application-specific class**, on a device with class 0, 0xff, or 0xfe. No macOS
driver may own that interface. A USB serial adapter or development board might
qualify; many composite boards and driver-owned adapters do not. Eligibility is
not a compatibility certification. Another application's exclusive claim can
still prevent opening an eligible device.

**Mac USB export has not passed a physical transfer test.** Local inventory was
verified against 11 real devices on an Apple Silicon Mac running macOS 26. None
was eligible for this first export scope. Protocol tests use an explicit fake
USB communicator; worker lifecycle tests use an explicit process fixture.

## What is not available

- Receiving a remote USB device in macOS. This needs both Apple's managed
  `com.apple.developer.usb.host-controller-interface` entitlement and a native
  import implementation. Developer Program membership does not include it
  automatically. See the [application draft](macos-apple-entitlement.md).
- Bluetooth, disk, keyboard/mouse, network, audio/video, composite-interface or
  isochronous export from a Mac. The Linux/Windows device policies are unchanged.
- Seizing devices from macOS drivers, disabling SIP, or installing a kernel
  extension. None is part of the installation flow.
- A verified Intel package or notarized public installer. The initial package
  targets Apple Silicon and macOS 14 or newer; only macOS 26 has been checked on
  the development Mac so far.

## Build and open

On an Apple Silicon Mac with Xcode Command Line Tools, Swift 6, Rust 1.97, and
Python 3.12 or newer:

```sh
git clone https://github.com/cobanov/portrelay.git
cd portrelay
./macos/build.sh
```

The script verifies a pinned MIT upstream archive, applies PortRelay's patches,
and builds `target/macos/package/PortRelay.app`. Copy the app to `/Applications`
or `~/Applications`, then open it. It opens the local browser window; there is
no menu-bar UI yet. A locally built, ad-hoc-signed app is for development. Do not
turn off Gatekeeper or SIP to install someone else's unnotarized download.

In the window:

1. Add your Linux or Windows computer using an invitation and approve it.
2. Plug in a dedicated test adapter. Choose **Share device** if it is eligible.
3. On Linux or Windows, choose **Connect**. **Disconnect** or **Stop sharing**
   ends the session. Losing the agent closes the worker's pipe and releases its
   IOKit handles when the worker exits.

Hardware compatibility, unplug/replug behavior under load, and loss of power
still need a physical two-computer acceptance test. Do not use this preview for
firmware updates or a production device.

## Terminal use

The CLI is inside the app:

```sh
"/Applications/PortRelay.app/Contents/MacOS/portrelay" desktop --no-open
"/Applications/PortRelay.app/Contents/MacOS/portrelay" menu
```

For a headless shell without a GUI login session, use `run --no-open` in a
persistent terminal session instead of `desktop`. The [terminal guide](terminal.md)
covers pairing and sharing. On Mac, `check` checks the export worker, **not** a
virtual USB import driver. `status` exposes separate `usb_export`, `usb_import`,
and `import_reason` capabilities. Attempting to connect a remote device on Mac
fails before requesting a lease on its owner.

## Signing and notarization

Release builders may set `PORTRELAY_SIGN_IDENTITY` to an existing Developer ID
Application identity and `PORTRELAY_NOTARY_PROFILE` to an existing `notarytool`
Keychain profile. `PORTRELAY_SIGN_KEYCHAIN` can select a specific keychain when
duplicate identities exist. During signing only, its search list is isolated
and then the exact original list is restored, including on command failure.
The build script signs each executable and the app, submits
the archive, staples the ticket, and verifies Gatekeeper acceptance. Without a
profile, it explicitly reports that the output is **not notarized**. No secrets
belong in the repository or shell history.

A developer who needs to save a profile can run this interactively in their
own terminal and enter the requested credentials there:

```sh
xcrun notarytool store-credentials portrelay
```

The USB entitlement request is separate from notarization. No entitlement
request has been submitted by this project yet.

## Remove the background app

```sh
launchctl bootout "gui/$(id -u)/dev.cobanov.portrelay"
rm ~/Library/LaunchAgents/dev.cobanov.portrelay.plist
```

Then remove PortRelay.app. Pairing state remains in
`~/Library/Application Support/dev.cobanov.portrelay`; keep it to retain trusted
computers or remove it separately to reset the installation.
