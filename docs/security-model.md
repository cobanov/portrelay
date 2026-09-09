# Implemented security boundaries

This describes the developer alpha, not an independent security audit.

- iroh 1.1.0 supplies QUIC/TLS and authenticated endpoint identities. A 32-byte
  random identity is stored in the current user's protected application data
  directory. Configuration and local API credentials are written atomically.
- Invitations contain a 256-bit random secret, the expected endpoint identity,
  connection addresses, a protocol version, and a ten-minute expiry. The owner
  consumes a matching invitation once and requires local approval of the new
  peer. Invitations are bearer secrets and must be exchanged privately.
- Inventory requires approval. A device also needs a grant for the authenticated
  peer and the current device generation. Linux device identity includes the boot ID,
  kernel device instance, bus/device number, and a hash of identifying data.
  USB serial strings are not exposed in the API or device protocol.
- One export lease is acquired atomically per device. Removing trust or sharing
  permission cancels streams. Connection acquisition, helper failure, stream EOF,
  and process shutdown have cleanup paths. Native helper recovery records are
  separate from user sharing intentions and are reconciled at helper restart.
- Control frames are length-prefixed JSON capped at 64 KiB. The protocol has no
  arbitrary shell command or network-destination proxy operation. Pairing is
  limited to 30 attempts/minute, 8 invitations, 64 known peers, 16 device
  sessions, and 32 simultaneous incoming transport tasks. First frames and
  connection establishment have deadlines. State-changing 0-RTT is not enabled.
- The Linux root helper accepts a mode-0600 Unix socket owned by the configured UID,
  inside a root-owned directory. It verifies the peer UID. It uses typed
  operations and fixed sysfs paths; it does not evaluate shell fragments.
- Existing distribution `usbip` executables bind/unbind Linux devices. Connected
  kernel socket descriptors carry USB/IP URBs through a bounded local bridge;
  no `usbipd` server is started. A temporary loopback listener only constructs a
  verified socket pair and closes before USB data starts. The installer also
  limits the helper's IP traffic to localhost with systemd.
- The Windows LocalSystem helper uses private named pipes. The control pipe
  accepts only the configured owner and SYSTEM; raw USB/IP is SYSTEM-only.
  The agent authenticates the pipe server PID through Service Control Manager.
  Native import verifies the exact kernel-owned TCP tuple and restricts the
  dedicated virtual controller to SYSTEM and administrators. Windows device
  generations and managed-return permissions are described in
  [ADR 0003](adr-0003-windows-alpha.md).
- The helper rejects already-managed USB/IP devices and protects hubs, input,
  storage, imported devices, and recognized network interfaces. A Bluetooth loan
  needs explicit acknowledgement in the local application. Kernel metadata and
  device availability are checked again before export.
- The local HTTP service binds only to IPv4 loopback. API calls require a random
  bearer token; foreign browser origins are rejected. The token enters the UI
  through a URL fragment, is removed from the displayed URL, and is kept in
  tab-local session storage. The UI has a restrictive CSP and no remote scripts,
  fonts, analytics, or device browser APIs. The public marketing website remains
  a separate simulation and does not connect to this local API.

## Package setup authorization

The Debian/Ubuntu package ships a fixed root-owned setup script and a polkit
policy scoped to that exact executable. The local authenticated API may request
setup, but an OS administrator must authenticate for every attempt. The target
owner UID comes from `PKEXEC_UID`, not from the API. Setup accepts no shell or
package-name input, validates its distro and kernel, installs distribution USB
support if needed, and enables the fixed helper unit. It never shares a device.
Only one local USB owner is accepted; a second UID cannot silently replace it.

The user service allows `pkexec` privilege transitions so it can request this
separate OS authorization. It is not a root service. The device helper retains
`NoNewPrivileges`, private runtime storage, UID-authenticated IPC, and its systemd
filesystem/network restrictions. No password enters the browser or agent API.

## Windows setup authorization

Windows setup uses the native UAC prompt to elevate a fixed packaged setup
script. It keeps the original user's SID as USB owner when a different
administrator approves. No password enters the browser or agent API. The
installer allows only the encrypted agent's UDP traffic; it does not expose
the device service or local HTTP API. See [Windows setup](windows-alpha.md).

## Remaining limits

A trusted USB device can attack an operating-system driver. Network encryption
cannot make malicious hardware safe. Physical compatibility, suspend/resume,
long-running transfers, controller firmware behavior, and recovery on additional
kernels are not covered by unit tests. An isolated kernel gadget is useful
integration evidence but is not a physical-device certification.

Hostile local processes running as the same user can control that user's agent
and access its helper. Root or Windows administrators can inspect device traffic. These are local trust
boundaries, not sandboxed adversaries. Pairing records and device permissions are
not automatically synchronized between installations. There is no automatic
reconnect, update service, public relay operation, or telemetry in the agent.

A recovery error remains a recovery error. The helper retains its journal,
reports the error to the local UI, and rejects new device operations until it is
resolved. Do not remove recovery records to make status appear healthy.
The Windows validation record also tracks an unresolved native controller/PnP
stall after repeated imports and a forced app shutdown. Empty application
sessions or journals do not establish complete kernel resource reclamation.

## Keyboard and mouse sessions

Input permission is separate from peer approval and USB device grants. Only the
configured user's unlocked active Linux desktop may receive control. A separate
root helper accepts that UID on a private local socket, exposes no network
listener and creates only virtual input devices. It bounds/validates batches
and removes virtual devices on closure. The browser sends only while its
explicit control surface is focused. See [ADR 0005](adr-0005-input-control.md).

## Account-managed trust (alpha.8)

GitHub sign-in uses device-signed challenges, state/cookie binding, S256 PKCE, a
CSRF-protected named-computer confirmation, and hashed per-device bearer tokens.
The registry stores identity/address metadata, never USB or input payloads.
Automatic trust covers computers in the same numeric GitHub account; device/input
grants remain separate. Remote removal propagates on the next 15-second check.
A monotonic 90-second lease closes managed connections during registry loss, and
restart requires fresh membership. Manual approval cannot override account expiry.
The registry is a trusted membership authority. See [ADR 0006](adr-0006-accounts.md).
