# ADR 0004: Limited native macOS export before USB import

Date: 2026-09-09. Status: implemented as a development preview; physical export
compatibility and public notarized distribution remain unvalidated.

Reuse the MIT USB/IP core from
[usbipd-mac](https://github.com/beriberikix/usbipd-mac/tree/1c2ab4594653db5859d6773bdd010303a641e98e).
The revision and archive SHA-256 are pinned in `macos/prepare.py`. Preserve its
license in source and app resources. The dependency lives under generated
`target/macos`; no upstream TCP server, installer, or System Extension is run.

PortRelay's Swift executable enumerates IORegistry without opening interfaces.
An authenticated, authorized device lease starts a normal-user worker with the
exact device generation. The worker rechecks eligibility and claims IOKit
interface 0 before emitting Ready. JSON control frames and subsequent USB/IP
bytes travel on inherited stdin/stdout, bridged into the existing QUIC session.
There is no raw USB/IP listener. Unexpected worker exits are recorded as failed
sessions. EOF closes the worker even after agent SIGKILL; pending IOKit handles
are process-owned. No local driver is unbound or persistent device state written.

Upstream patches are deliberately narrow:

- Bind actual IOKit lookup to the selected IORegistry entry ID, rather than the
  first VID/PID match. Generation also includes the boot session and entry ID,
  so a replacement adapter cannot inherit an old grant.
- Expose a request-admission callback to order UNLINK after SUBMIT registration.
  PortRelay queues each endpoint in wire order, lets other endpoints progress,
  and cancels queued work before it reaches USB.

Limits: 1 MiB per transfer, 64 outstanding requests with 16 additional slots for
cancellation, a 15-second operation timeout, no isochronous packets, and no more
than 32 worker leases per agent. Mac bus numbers are locationID's bus byte plus
one because the existing receiving backends reject bus zero. Up to four port
nibbles form the low 16-bit device number; longer paths fail closed.

Eligibility requires exactly interface 0, vendor/application-specific interface
class, compatible device class, a recognized speed and no interface driver.
This reflects the upstream communicator's interface-0 assumption. Broadening
classes before multi-interface ownership and handoff exist would overstate
support and risk taking over the wrong device.

Mac import is an independent role. The agent rejects it before contacting the
owner; UI and status explain why. Apple's managed host-controller entitlement
and a licensed native import implementation are still required. No code from
unlicensed experimental import repositories is included.

Distribution uses a normal .app and per-user LaunchAgent. The bundle opens the
existing local browser UI. Signing and optional notarization are scripted;
credentials stay in Keychain. The first build is Apple Silicon. Intel and older
macOS versions need their own execution and installation evidence.
