# Roadmap

The product scope is Linux, Windows, and macOS.

The Linux and Windows developer backends implement encrypted native USB paths. See
[validation](validation.md) for kernel-fixture evidence and remaining limits.
Physical USB/Bluetooth compatibility and the complete non-technical setup gate
remain open. Completed code is not a claim of universal device support.

## 0. Project bootstrap

- [x] Create the local project directory and public GitHub repository.
- [x] Add an open-source license and contribution guidance.
- [x] Research upstream USB and Bluetooth implementations before writing code.
- [x] Record initial architecture, platform limitations, and acceptance gates.

## 0.1. Product website and interface concept

- [x] Build a static product website with the planned three-step setup in the hero.
- [x] Add an explicitly simulated desktop interface for remote connection,
      local sharing, occupied devices, and paired-computer selection.
- [x] Explain Linux, Windows, and macOS development status without fake installers.
- [x] Add a portable static build and Sites configuration for an owner-only preview.
- [x] Redesign around visual printer, Bluetooth-adapter, and USB-drive examples.
- [x] Configure public Cloudflare Pages hosting at portrelay.cobanov.dev.

The website demo does not provide hardware validation. The separate Linux
application below supplies the implemented device path.

## 1. Prove the device path

- [x] Create the Rust agent/CLI workspace and pin dependency versions.
- [x] Implement Linux USB inventory, backend detection, and explicit capability
      reporting without detaching devices during discovery.
- [x] Prove one approved device can pass from Linux export to Linux native
      import through a restricted local bridge and encrypted connection
      (kernel virtual serial fixture; physical hardware remains pending).
- [x] Implement pairing, device permissions, exclusive leases, revocation,
      bounded I/O, and failure cleanup in that same path.
- [ ] Verify a dedicated USB Bluetooth adapter can be lent, pair to a nearby
      device from the receiving OS, and return to its original owner.
- [ ] Record device identity, both OS/kernel versions, backend versions, real
      application behavior, and remaining limitations for each result.

Exit gate: a normal application uses a real remote USB device; a remote
Bluetooth adapter is tested as a separate capability; unauthorized peers and
concurrent claims are rejected; disconnect and owner recovery are verified.

## 2. Make two-computer LAN setup understandable

- [x] Add an embedded local control window using the same agent as the CLI.
- [ ] Package a native desktop shell if needed; Tauri deferred in ADR 0002.
- [x] Add invitation entry and paired-computer management.
- [x] Add GitHub account sign-in, automatic membership and address refresh,
      with per-computer removal and a headless login link (alpha.8).
- [ ] Add multicast discovery for independent account-free LAN use.
- [x] Build first-run setup, dependency checks, local device sharing, remote
      device connection, busy state, and owner reclaim flows.
- [x] Add permission, unplugged, missing-driver, blocked-network, and restore
      failure messages with actionable recovery.
- [x] Provide Debian/Ubuntu packages, automatic dependency setup, an app-menu
      launcher, and a guided first-run sharing flow.
- [x] Add computer naming and invitation copy/file import without exposing
      advanced connection details in the main flow.
- [ ] Validate installation and menu launch on a fresh graphical Linux desktop.
- [ ] Observe a non-technical user complete first sharing without CLI commands.

Exit gate: both computers can be installed and paired, and a tested device
connected and returned entirely through the application on an isolated LAN.

## 3. Internet operation

- [x] Add identity-bound invitation import/export and expiry handling.
- [x] Test a forced external relay with direct IP transports disabled.
- [ ] Test direct connectivity across two independent NATs.
- [ ] Publish and test a self-hosted relay deployment with configurable endpoints.
- [ ] Measure latency, throughput, resource limits, and behavior under packet
      loss for the tested device classes. Clearly identify relayed sessions.
- [ ] Validate revoked invitations, hostile frames, stalled streams, and relay
      outages. Confirm the raw USB/IP backend is unreachable from the network.
- [ ] Define sustainable capacity and abuse controls before offering any shared
      public service as the default.

Exit gate: the same tested USB and Bluetooth-adapter paths work across two real
networks, including an explicitly forced relay path. LAN still works without
public services. Do not infer latency-sensitive audio/video support from a
successful storage or serial test.

## 4. Windows integration

- [x] Pin and validate upstream export and import backends independently.
- [x] Load production-signed drivers on Windows 11 x64 with Secure Boot enabled.
- [ ] Validate additional Windows versions and CPU architectures.
- [x] Restrict upstream network listeners and validate local bridge isolation.
- [x] Implement helper/service lifecycle and correct device restoration.
- [x] Validate Windows/Linux in both directions with native virtual serial devices.
- [ ] Validate Windows/Windows on two computers.
- [x] Publish the Windows 11 x64 installer; exercise native UAC, login startup,
      package replacement, and clean-state uninstall with Secure Boot enabled.
- [ ] Resolve the upstream controller stall during removal after repeated imports
      and forced app shutdown. Extend removal/recovery tests to more Windows systems.

## 5. macOS and individual Bluetooth devices

These are separate deliverables; neither is implied by a cross-platform UI.

- [ ] Implement a BLE bridge with service discovery, read/write, subscriptions,
      pairing errors, reconnect behavior, and documented compatible consumers.
- [ ] Test duplicate service UUIDs, identifier differences between OSes,
      notification backpressure, and peripheral reconnection.
- [ ] Decide which explicit profile adapters can provide useful integration
      with existing applications without claiming universal virtualization.
- [ ] Evaluate Bumble/HCI for Linux and controller test infrastructure.
- [x] Integrate a pinned Mac USB/IP core, read-only inventory, instance-bound
      worker, role-aware UI/CLI, and app packaging (development preview only).
- [x] Prepare the Apple host-controller entitlement request for account-owner review.
- [ ] Prove limited macOS export with claimable serial adapters and development
      boards using usbipd-mac, with SIP enabled.
- [ ] Resolve the separate macOS import entitlement gate before promising remote
      USB attachment on a Mac. No release date is committed for this role.
- [x] Package, Developer ID sign, and notarize the Apple Silicon export preview.
- [ ] Validate macOS BLE permissions and implement the separate Bluetooth role.

## Release evidence

Automated tests will cover framing, authentication and authorization failures,
lease contention, lifecycle cleanup, cancellation, and resource bounds. Linux
virtual USB fixtures may help test behavior in CI, but physical device results
remain separate. Builds on three operating systems are not compatibility tests.

A release candidate needs a recorded hardware matrix: USB serial, an input
device, disposable test storage, and a dedicated Bluetooth adapter as initial
candidates. For storage, compare a known test file before/after transfer and
exercise disconnect recovery without valuable data. Add isochronous audio/video
only after their own latency and recovery tests.

Each claimed platform/device pair also needs hotplug, abrupt peer termination,
network loss, suspend/resume, permission denial, owner reclaim, and clean
uninstall results. Publish the failures and exclusions alongside successes.

## Alpha.4 device handoff

- [x] Replace blanket input/storage/network blocks with explicit handoff consent
  and disk/network usage gates checked again by the helper.
- [x] Exercise native Linux mass-storage read/write/return, HID keyboard/mouse,
  and CDC Ethernet using isolated virtual fixtures.
- [x] Group a hub's currently connected devices without granting future hotplugs.
- [x] Guide whole-adapter Bluetooth handoff and open receiving-system settings.
- [ ] Validate physical USB Bluetooth pairing and peripheral use.
- [ ] Validate the new classes on native Windows, including recovery.
- [ ] Resolve the recorded upstream Windows controller-removal stall.

## Alpha.5 ARM installation

- [x] Build ARM64 and ARMv7 packages with architecture and executable-version checks.
- [x] Detect Raspberry Pi OS / Debian 12–13 and Ubuntu 24.04 in the bootstrap.
- [x] Add explicit headless setup with a non-root agent and startup without login.
- [x] Run native ARM64 and emulated ARMv7 application tests.
- [ ] Record physical Raspberry Pi USB handoff, Bluetooth pairing, and recovery.
- [x] Replace advanced JSON terminal actions with a simpler interactive flow (alpha.6 menu and short commands).

## Focused input control (alpha.7 / Mac preview.2)

- [x] Mac app window sends keyboard/mouse events over authenticated transport.
- [x] Separate input grants, exclusive sessions, bounded batches and watchdogs.
- [x] Restricted Linux uinput helper and optional desktop/terminal setup.
- [x] Mac to ARM Linux kernel acceptance and Chrome start/input/Escape flow.
- [ ] Visible receiving-app acceptance, lock transitions, crash stress and latency.
- [ ] Additional browser, Wayland, Windows sender and keyboard-layout acceptance.
- [ ] Native global capture, screen-edge switching and Mac/Windows input receivers.

These are separate from whole USB HID and Bluetooth device sharing.
