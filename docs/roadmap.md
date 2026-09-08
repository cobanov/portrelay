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
- [ ] Add automatic local peer discovery and address refresh.
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
- [ ] Package, upgrade, and uninstall on clean Windows systems without enabling
      test-signing or disabling Secure Boot.

## 5. macOS and individual Bluetooth devices

These are separate deliverables; neither is implied by a cross-platform UI.

- [ ] Implement a BLE bridge with service discovery, read/write, subscriptions,
      pairing errors, reconnect behavior, and documented compatible consumers.
- [ ] Test duplicate service UUIDs, identifier differences between OSes,
      notification backpressure, and peripheral reconnection.
- [ ] Decide which explicit profile adapters can provide useful integration
      with existing applications without claiming universal virtualization.
- [ ] Evaluate Bumble/HCI for Linux and controller test infrastructure.
- [ ] Prove limited macOS export with claimable serial adapters and development
      boards using usbipd-mac, with SIP enabled.
- [ ] Resolve the separate macOS import entitlement gate before promising remote
      USB attachment on a Mac. No release date is committed for this role.
- [ ] Validate macOS BLE permissions, packaging, signing, and notarization.

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
