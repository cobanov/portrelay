# Roadmap

This is an implementation plan, not a release announcement. There are no
published installers, tested device pairs, or scheduled release dates yet.

## 0. Project bootstrap

- [x] Create the local project directory and public GitHub repository.
- [x] Add an open-source license and contribution guidance.
- [x] Research upstream USB and Bluetooth implementations before writing code.
- [x] Record initial architecture, platform limitations, and acceptance gates.

## 1. Prove the device path

- [ ] Create the Rust agent/CLI workspace and pin dependency versions.
- [ ] Implement Linux USB inventory, backend detection, and explicit capability
      reporting without detaching devices during discovery.
- [ ] Prove one approved device can pass from Linux export to Linux native
      import through a restricted local bridge and encrypted connection.
- [ ] Implement pairing, device permissions, exclusive leases, revocation,
      bounded I/O, and failure cleanup in that same path.
- [ ] Verify a dedicated USB Bluetooth adapter can be lent, pair to a nearby
      device from the receiving OS, and return to its original owner.
- [ ] Record device identity, both OS/kernel versions, backend versions, real
      application behavior, and remaining limitations for each result.

Exit gate: a normal application uses a real remote USB device; a remote
Bluetooth adapter is tested as a separate capability; unauthorized peers and
concurrent claims are rejected; disconnect and owner recovery are verified.

## 2. Make two-computer LAN setup understandable

- [ ] Add the Tauri desktop shell using the same agent as the CLI.
- [ ] Add local peer discovery, manual fallback, and paired-computer management.
- [ ] Build first-run setup, dependency checks, local device sharing, remote
      device connection, busy state, and owner reclaim flows.
- [ ] Add permission, unplugged, missing-driver, blocked-network, and restore
      failure messages with actionable recovery.
- [ ] Package and uninstall on a fresh Linux desktop with no developer tools.
- [ ] Observe a non-technical user complete first sharing without CLI commands.

Exit gate: both computers can be installed and paired, and a tested device
connected and returned entirely through the application on an isolated LAN.

## 3. Internet operation

- [ ] Add identity-bound invitation import/export and expiry handling.
- [ ] Test direct connections across separate NATs and forced relay fallback.
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

- [ ] Pin and validate upstream export and import backends independently.
- [ ] Verify production driver signatures and installer behavior on supported
      Windows versions and CPU architectures.
- [ ] Restrict upstream network listeners and validate local bridge isolation.
- [ ] Implement helper/service lifecycle and correct device restoration.
- [ ] Validate Windows/Linux in both directions, then Windows/Windows.
- [ ] Package, upgrade, and uninstall on clean Windows systems without enabling
      test-signing or disabling Secure Boot.

## 5. Individual Bluetooth devices and macOS

These are separate deliverables; neither is implied by a cross-platform UI.

- [ ] Implement a BLE bridge with service discovery, read/write, subscriptions,
      pairing errors, reconnect behavior, and documented compatible consumers.
- [ ] Test duplicate service UUIDs, identifier differences between OSes,
      notification backpressure, and peripheral reconnection.
- [ ] Decide which explicit profile adapters can provide useful integration
      with existing applications without claiming universal virtualization.
- [ ] Evaluate Bumble/HCI for Linux and controller test infrastructure.
- [ ] Resolve macOS USB export and import permissions separately and prove both
      paths with SIP enabled before offering them.
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
