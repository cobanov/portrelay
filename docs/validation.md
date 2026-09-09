# Alpha validation record

Dates: 2026-09-08 and 2026-09-09. Current version: `0.1.0-alpha.6`.

The kernel device tests below were recorded for alpha.1. Alpha.2 changes the
installer and onboarding; the device transport/helper backend is unchanged.

## What works in the recorded setup

An unprivileged PortRelay agent on two Linux computers, each with the restricted
root helper, carries a USB serial device through the native USB/IP and virtual
host controller drivers. A normal process opens the resulting `/dev/ttyACM*`
and exchanges bytes. This is kernel device traffic, not a mocked byte stream.

The source device was an explicitly created **virtual** ACM serial gadget using
Linux `dummy_hcd` and configfs, VID:PID `1d6b:0104`. It was created in a disposable
Debian VM, not bound from physical hardware. Production export code uses the
same `usbip-host` backend and distribution USB/IP executable.

- Exporter: Debian 13, x86_64, kernel `6.12.107+deb13-amd64`, distribution `usbip`.
- Importer: Ubuntu 24.04.4, x86_64, kernel `7.0.0-30-generic`, `vhci-hcd`.
- Transfer: 65,536 bytes, sent and echoed in 512-byte blocks.
- SHA-256: `7daca2095d0438260fa849183dfc67faa459fdf4936e1bc91eec6b281b27e4c2`.
- Direct QUIC: exact payload match, disconnect, reconnect, and owner revocation.
- Repetition: 20 additional direct connect/disconnect cycles passed.
- Forced relay: exact payload match through an external upstream development
  iroh relay, with **all direct IP transports disabled on both agents**.
  Three additional relayed connect/disconnect cycles and owner revocation passed.
- Each clean disconnect checked both agent session lists, imported-device
  removal, empty helper recovery journals, healthy helpers, and restoration of
  the source device's `usb` driver. No lingering USB/IP stub workers remained.
- Abrupt helper termination: `SIGKILL` on the disposable exporter's helper,
  systemd restart, journal reconciliation, original driver restoration, and a
  subsequent new connection passed over the relay.
- Abrupt importing-agent termination: `SIGKILL` closed the imported device;
  the exporter restored its original driver and both recovery journals cleared.

The direct fixture used an explicit UDP forward into the disposable VM. The
relay test used an actual external internet relay, but the endpoints shared an
upstream connection. This does not prove direct hole punching across two
independent NATs, geographic latency, or production relay availability.

## Windows native USB evidence (alpha.3)

Windows tests used a fresh disposable Windows 11 Enterprise evaluation VM,
version `10.0.26200`, x64, with **Secure Boot enabled and TPM 2.0 ready**. No
physical workstation was modified and no signature-enforcement setting was
relaxed. The app ran as a standard user; its separate helper ran as LocalSystem.
The original signed usbip-win2 0.9.8.0 and Oracle 7.2.2 drivers loaded normally.

- **Windows → Ubuntu:** QEMU's virtual FTDI serial device (`0403:6001`) was
  exported through PortRelay's private usbipd-win service. Ubuntu 24.04,
  kernel `6.8.0-139-generic`, enumerated it through `vhci_hcd` and `ftdi_sio`
  as `/dev/ttyUSB0`.
- **Debian → Windows:** the `dummy_hcd` ACM gadget (`1d6b:0104`) on Debian 13,
  kernel `6.12.107+deb13-amd64`, appeared as a native Windows COM port using
  the inbox USB serial driver. No WSL or Linux guest ran inside Windows.
- Both directions exchanged and echoed **65,536 bytes** in 512-byte blocks,
  SHA-256 `7daca2095d0438260fa849183dfc67faa459fdf4936e1bc91eec6b281b27e4c2`.
  The Windows receiving process was an ordinary, non-administrator account.
- Both directions passed five additional connect/disconnect cycles and active
  owner revocation. Unapproved peers, devices without a grant, and concurrent
  second claims were rejected. Normal closure left no new session errors.
- Both directions also passed the same 65,536-byte transfer, two additional
  connection cycles, and revocation through an external iroh relay with direct
  IP transports disabled on every agent. The endpoints shared an upstream
  network; this does not establish connectivity across independent NATs or
  production relay availability.
- Every completed test checked empty agent sessions, removal of the imported
  serial port, and healthy helpers. Privileged recovery journals and the original
  Windows device driver were also checked after the dedicated recovery tests.
- Removing and adding the virtual FTDI fixture again at the **same port with
  the same serial number** changed its generation. The old grant no longer
  listed it remotely and a stale connect request was rejected.
- Killing the Windows helper during an active export exercised Windows service
  restart, original driver restoration, journal reconciliation, and a subsequent
  successful new connection and disconnect.
- Killing the Windows importing agent during an active relayed connection
  removed the COM port, returned the Debian gadget, and cleared both helpers'
  recovery journals without stopping the Windows helper.
- A different local account could not open the control pipe. The standard owner
  could use the app but could not open the native USBip controller directly.
  The service verified the live controller DACL and the accepted kernel TCP
  tuple before native import. No TCP 3240 listener was present.

The direct tests used explicit UDP forwards between disposable NAT guests;
this is controlled network interoperability, not automatic NAT traversal proof.
These are **virtual USB devices using real OS drivers**, not physical-device
compatibility results. Windows-to-Windows and physical Bluetooth pairing remain
separate acceptance gates. `tests/windows-smoke.py --help` documents the real
Windows/Linux integration harness and requires explicit disposable-host confirmation.

Issues found and corrected during these tests:

- Default Windows service access and SYSTEM-token queries did not work for a
  normal non-interactive user. The app now authenticates the pipe PID through
  SCM, with query-only permission for the configured owner.
- The USB export monitor needed an explicit service dependency to start before
  the helper, including after reboot.
- QUIC reports a successful peer close through an I/O error wrapper. The agent
  distinguishes its explicit success code/reason from real network/helper
  failures. A regression test covers wrapped success, failure, and timeout cases.
- Native import can disappear after EOF just before explicit detach. Cleanup
  re-queries the exact owned URL before accepting an already-removed port.
- Windows re-enumerates a returned device. For a system-wide unique identity
  with unchanged USB/IP descriptors, only that managed return retains its grant;
  a later arrival or helper restart invalidates it. Other devices require re-sharing.

An unresolved issue appeared after the repeated imports and importing-agent
crash test: upstream `devnode.exe` stalled in `DiUninstallDevice` for the dedicated
USBip controller. Guest shutdown also stalled; a reset of that disposable guest
followed by retry completed removal. The uninstaller now bounds that wait and
preserves ownership/progress for a retry. This is **not a fix for the kernel/PnP
stall**, and the successful transfer/port-removal checks do not establish complete
driver resource reclamation. Windows reliability and clean removal after every
failure mode remain open acceptance gates.

## Installation evidence

### Windows (alpha.3)

The generated Inno Setup `.exe` installed on the disposable Windows 11 VM,
without Rust, .NET SDK, WSL, or separately downloaded product drivers. The
packaged Start-menu shortcut was present and the desktop agent ran in the
standard user's interactive login session.

- The real `setup_usb` API action opened Windows' native UAC credential window.
  Cancelling left USB disabled with a retryable error; retrying and approving
  installed the signed drivers and a healthy LocalSystem helper for the original
  standard user's SID, not the administrator who entered the password.
- Secure Boot and TPM remained enabled. No raw TCP 3240 listener was present.
- The packaged installation then passed a 65,536-byte transfer, one additional
  connection cycle, and revocation in each Windows/Linux direction.
- A normal guest reboot after those transfers restored the desktop agent and
  helper automatically, with the same computer identity and no active sessions.
  Replacing the installed package with the final CI installer completed
  successfully and restarted the helper while retaining the user configuration.
  That update requested a Windows restart for in-use files. The final package's
  uninstaller subsequently completed with exit code 0 and removed the app,
  configured services, driver client, firewall rule, and loaded owner's startup
  entry. This successful clean-state removal does not close the earlier PnP stall.
- A firewall prompt on first launch was found and corrected by preparing only
  the encrypted UDP rule during elevated package installation, before launching
  the agent. The USB setup still uses the normal administrator consent window.

The package was installed silently for reproducibility; graphical installer
button clicks and browser controls have not been exercised. Native UAC was
observed and operated on the actual Windows secure desktop. This is not a
non-technical-user usability result. The controller-removal stall described
above remains an unresolved reliability limitation.

### Debian and Ubuntu (alpha.2)

Alpha.2 provides separate Debian and Ubuntu `.deb` packages. The binary is built
in Rust 1.97's Debian Bookworm image (glibc 2.36 baseline). Fresh disposable VMs
have no Rust or JavaScript toolchain:

- Debian 13.6, kernel `6.12.107+deb13-amd64`, distribution `usbip`.
- Ubuntu 24.04.4, kernel `6.8.0-138-generic`. Setup installed the missing matching
  `linux-tools` and `linux-modules-extra` packages automatically.
- `apt install` resolved package dependencies. `portrelay desktop --no-open`
  enabled the user service and reached the authenticated API.
- The actual `setup_usb` API action invoked the packaged `pkexec` policy.
  A terminal polkit authentication agent registered for the running application
  exercised administrator-password approval and cancellation. Cancellation left
  USB disabled; retry with authentication started a healthy helper on both VMs.
  No permissive polkit test rule or password-handling API was used.
- The root setup script rejected a different local owner UID; helper socket
  access from that other user was denied. No raw TCP 3240 listener was present.
- Package replacement stopped/restarted the configured services. Guest reboot
  followed by login restored agent/helper readiness. Computer name and identity
  survived. The user service starts at login; lingering is not enabled by default.
- Removing and purging the package stopped services and removed the executable
  and system owner configuration, while retaining user identity/pairing files.
  No device recovery journals remained in these installation-only fixtures.

`tests/installer-smoke.py` reproduces API setup approval/cancellation on a fresh
installed package using a password-enabled administrator test account. It requires
`--confirm-disposable`, `--password-file`, and the `pexpect` test dependency. Never
run it on a production workstation. No passwords are printed or sent to PortRelay.

These are **headless** installation and authorization tests. Applications-menu
clicks, graphical password-dialog rendering, invitation clipboard/file controls,
and non-technical user usability have not been observed. The `.desktop` launcher
and guided UI are implemented, but that complete graphical acceptance gate stays
open. The older manual tarball installer also passed Debian installation,
reboot, helper-isolation and uninstall checks in alpha.1.

Packages include checksums, license notices, `Cargo.lock`, and a CycloneDX SBOM.
They are unsigned experimental packages, not a signed stable release.

## Website and one-command installers (2026-09-09)

The static website was observed in a browser at 320, 390, 680, 681, 768, 1024,
and 1440 CSS pixels. Desktop/tablet demo-card edges align, mobile device actions
remain in their rows, and the page has no horizontal overflow at those widths.
Screenshots cover desktop and mobile layouts, including connected device states.
Printer, Bluetooth-adapter, USB-drive, local sharing, and OS-selection concepts
were exercised. Both installer commands were copied and pasted into a local test
field and matched their source strings exactly. These remain simulated devices.

[Bootstrap CI run 34282750543](https://github.com/cobanov/portrelay/actions/runs/34282750543)
passed against the published alpha.3 packages:

- A fresh Ubuntu 24.04 runner executed the shell bootstrap through a pipe,
  installed the package, repeated installation, and removed it using apt.
- Debian 13 and Ubuntu 24.04 downloaded the correct release asset and verified
  its pinned SHA-256. Corrupt downloads and unsupported ARM architecture were
  rejected, and temporary downloads were removed.
- Windows PowerShell 5.1 verified the release installer, installed it, executed
  the public `Invoke-Expression` path for a repeat install, and uninstalled it.
  Corrupt downloads, ARM64, and the runner's real Server platform were rejected.
  GitHub's Windows runner uses Server, so the successful bootstrap test supplies
  a Client OS-detection fixture and adds silent installer flags in the test only.
  It does not extend support to Server or replace a Windows 11 graphical/UAC test.

The bootstrap scripts select a fixed version and hash; they are not an update
service or publisher signatures. Native binaries, USB setup, and device-support
boundaries are unchanged. No new physical-device or zero-terminal desktop
acceptance is claimed by these checks.

## Automated checks

Linux has 13 Rust tests covering protocol size/path validation, invitation
expiry and single use, real QUIC pairing and approval, authorization and exclusive
leases, local API authentication/origins, private TCP socket construction,
virtual controller status parsing, encrypted streams, cancellation, and backend
failure propagation, and computer-name persistence across restarts. Nine portable tests also pass on macOS and Windows. Linux-only tests
are explicitly gated; a macOS test pass does not imply a USB backend exists.

Formatting, Clippy with warnings denied, local UI JavaScript syntax, website
syntax/build checks, and the packaged binary are checked separately. The test
suite's in-memory streams are distinct from the kernel tests above. The installed app's browser UI, real desktop-menu launch, and usability with a
non-technical person remain separate acceptance gates; the public website QA
is recorded above.

`cargo audit` against the advisory database on this date found **zero known
vulnerabilities** and one unmaintained-dependency warning: `paste 1.0.15`,
[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html), transitively
used by Linux network discovery dependencies in iroh. It is a build-time macro;
the warning remains tracked rather than suppressed. This is not an independent
security audit. Packages carry dependency versions and upstream notices.

## Corrections discovered during kernel testing

- A fast disconnect could race `usbip unbind` against the kernel's own session
  shutdown, leaving restoration incomplete. The helper now waits for the kernel
  export state before unbinding, verifies restoration, and retains failed
  recovery records. The agent waits for a helper acknowledgement before ending
  the lease. The repeated tests above use this corrected path.
- Import EOF can empty a virtual port just before the explicit detach write.
  Cleanup accepts that case only after verifying that the port is already empty.
- A fresh headless VM may not have `/sys/bus/usb` until modules load. The service
  sandbox now uses the existing `/sys/bus` parent so the helper can start at boot.

An earlier experimental `usbip-vudc` fixture on a Linux 7.0 source host completed
one byte transfer, after which that host became unreachable. Its root cause has
not been established and its recovery was not verified. That fixture/backend
has been removed from the application and test scripts. Subsequent kernel work
used a disposable Debian VM and the production `usbip-host` path. This incident
is an unresolved limitation, not a successful reliability result.

## Device class acceptance (alpha.4, 2026-09-09)

Two disposable x86_64 VMs, with ordinary non-root agents and separate root helpers:
Debian 13 / `6.12.107+deb13-amd64` exported configfs `dummy_hcd` fixtures to Ubuntu
24.04 / `6.8.0-124-generic`. QUIC used an explicit source-VM UDP forward. The test
used the production USB/IP backend; no raw USB/IP network listener was started.

- **Storage:** a virtual mass-storage disk (`1d6b:0105`) accepted 262,144 bytes
  through the receiver's normal mounted ext4 filesystem. After sync, unmount,
  disconnect and source-driver restoration, mounting the returned disk on the
  owner produced identical bytes. SHA-256:
  `2312394bd99545d9de131c24efb781e765ac1aec243f2ed9347597a793a415e9`.
- A mounted source disk was rejected when granting access. Mounting it **after**
  a grant also caused a new connection to be rejected by the privileged helper.
- **Keyboard/mouse:** a composite HID fixture (`1d6b:0106`) delivered native
  `EV_KEY KEY_A` press and release plus `EV_REL REL_X=5` through the receiver's
  normal input event devices.
- **Network:** a virtual CDC Ethernet adapter (`1d6b:0107`) carried three ICMP
  exchanges between its receiving-host interface and its gadget interface,
  with all three replies received. An administratively up source adapter was
  rejected; disabling it made the device eligible.
- Every risk-bearing fixture was rejected without its handoff acknowledgement.
  Every completed loan removed the receiving device, cleared both privileged
  recovery journals, and left both helpers healthy. The storage result also
  verified source-device restoration by actually reading its returned data.

These are **virtual devices with real drivers**, not physical compatibility or
Windows class-handoff evidence. The new Windows policy and USB/IP code build and
pass conformance tests. A read-only query on the Windows CI runner also verified
that the real Windows disk/network providers identify online disks and active
adapters as unsafe. This does not replace Windows USB class-handoff tests.
Bluetooth handoff now includes acknowledgement and a
receiving-OS pairing guide/settings launcher, but no physical adapter or paired
peripheral was available for this acceptance run. Hub grouping is an application
permission flow; it does not claim to transport hub hardware.

The real application HTML/JavaScript also passed a DOM-based fixture check for
hub grouping, denied/accepted consent, skipping blocked children, reporting a
partial group failure, Bluetooth guidance/settings action, and connected-device
status. This is not visual inspection: the Mac was locked during this run, so
the changed application screen has not received a new screenshot review.

The alpha.4 Linux release archives reuse the CI-built executable from successful
run `34287270065` (`b728e6e`). Packaging was rerun with `377b639` after finding
hardcoded alpha.3 package labels; packaging now derives the version from Cargo and
rejects a binary-version mismatch. The exact resulting Debian/Ubuntu packages
installed in both disposable VMs, reported `0.1.0~alpha.4`, and activated healthy
helpers through the packaged setup command. This did not exercise a GUI permission
prompt. Windows assets and corresponding source are from the same successful CI.

Reproduce with `tests/device-class-gadget.py` and `tests/device-class-smoke.py
--help`. The scripts require explicitly disposable VMs and never choose an
existing physical disk, input device, or network adapter. Ubuntu's tested source
kernel did not contain `dummy_hcd`; fixture generation used Debian's standard
kernel. This is a fixture requirement, not a restriction on physical USB export.

## ARM packaging and headless installation (alpha.5, 2026-09-09)

[Build and package CI 34329813712](https://github.com/cobanov/portrelay/actions/runs/34329813712)
passed all 15 jobs at `285fe25`. Released binaries/packages are the exact assets
from this run; all 20 uploaded release-file digests matched locally verified files.

Alpha.5 adds Linux ARM64 and ARMv7 packages alongside AMD64. Rust device transport
and USB policy code are unchanged from alpha.4. The packaging verifies the ELF
class/machine, ARM hard-float ABI, and executable version before naming an archive
or writing the Debian Architecture field. An ARM64 executable mislabeled AMD64
was rejected. All Linux builds retain the Debian Bookworm / glibc 2.36 baseline.

- Native ARM64 and QEMU-user ARMv7 each passed all 15 Linux Rust tests. The
  encrypted application and protocol tests use their real binaries; these tests
  do not attach physical USB hardware.
- Clean Debian 12 and 13 userspaces on **both** ARM64 and ARMv7 installed the
  actual `.deb`, resolved its dependencies, started the non-root agent, read
  authenticated state, created an invitation, and completed termination,
  reinstall, remove and purge. ARMv7 execution was emulated. Containers have no
  USB helper/kernel support, so these are userspace/package results only.
- Separate full Debian 13 VMs on AMD64 (`6.12.107+deb13-amd64`) and ARM64
  (`6.12.107+deb13-arm64`) passed headless USB setup, repeated setup, non-root
  process ownership, a healthy real USB helper, restarting the user manager
  without a login, package reinstall, removal, and purge. Root ownership was
  rejected. These tests load real kernel USB/IP modules but do not lend a device.
  The VMs and generated SSH keys were removed by the test harness.
- A clean Ubuntu 24.04 ARM64 userspace also passed the package/agent lifecycle
  checks. This does not validate its Pi-specific kernel modules.
- The bootstrap's detection fixtures cover Debian/Pi OS Bookworm and Trixie,
  Ubuntu 24.04, and a 32-bit OS on a 64-bit kernel. Real SHA-256 verification
  rejects corrupt downloads. Piped headless orchestration is also checked for
  normal/root callers and rejects missing/root/invalid owners before download. Unsupported distro/CPU combinations, including
  ARMv6, stop before download. These fixtures are not a booted Pi OS image.
- Headless setup explicitly enables the chosen regular account's systemd
  lingering and user service, then performs the same privileged USB setup.
  Root ownership is rejected. Devices remain private, and the control API stays
  authenticated on loopback. Account-wide lingering is intentionally preserved
  on uninstall; its manual removal is documented in the Pi guide.

The first headless CI run used the hosted Ubuntu runner's kernel, which lacked
USB/IP modules even after its distribution module package was installed. Setup
correctly refused to report USB ready. The test now boots isolated Debian guests
with the standard distribution kernel instead of changing the runner's kernel
or faking module availability. No physical device is selected by installer tests.

Reproduce with `tests/linux-package-userspace.sh`, `tests/headless-vm.sh`,
`tests/headless-install.sh`, and `tests/bootstrap-linux.py`. The VM/container tests
require their explicit disposable-environment flags. Raspberry Pi model-specific
USB handoff, real Pi OS boot, physical peripherals, suspend/resume, and Bluetooth
pairing still require hardware acceptance. Pi built-in Bluetooth forwarding is
not implemented. See [Raspberry Pi setup](raspberry-pi.md).

## Terminal controls (alpha.6, 2026-09-09)

[Build and package CI 34334586811](https://github.com/cobanov/portrelay/actions/runs/34334586811)
passed all 15 jobs at `3441efb`. The alpha.6 release uses these exact Linux,
ARM64, ARMv7 and Windows artifacts; all 20 uploaded file digests matched GitHub's
SHA-256 values. The existing clean userspace and full headless VM installation
checks passed for the new packages.

- `tests/terminal-cli.py` passed on native Linux AMD64/ARM64, Windows, and macOS.
  Two actual PortRelay processes used real authenticated local APIs and encrypted
  peer connections for invitation, single-use rejection, approval, remote listing,
  rename and revocation. No USB device was shared by this part of the test.
- Explicit HTTP fixtures exercise the actual CLI executable for named and ID
  selection, ambiguous names/prefixes, generation forwarding, device warnings,
  blocked/busy devices, ejection confirmation, and terminal-control sanitization.
  These fixtures are device-control UI evidence, not physical attachment evidence.
- Real pseudoterminals on Linux/macOS exercised invalid menu choices, cancellation,
  refused and accepted disk warnings, sharing, disconnection and EOF exit.
  Windows ran the direct command tests; a Windows terminal window was not visually
  inspected. The source also has Rust selection, bounded-input and display tests.
- The released ARMv7 `.deb` was additionally installed into a disposable emulated
  Debian 12 userspace. As a regular user, its packaged executable passed the same
  real-agent, HTTP-fixture and pseudoterminal acceptance flow.
- A grant regression test verifies that stale device selection cannot create or
  modify a grant, and that fresh selection still requires storage consent. The
  terminal and local app window now send the selected generation; legacy JSON
  actions without that optional field remain accepted for compatibility.
- Bootstrap distro/architecture, real-checksum rejection, cleanup, and piped
  headless/root-owner fixtures passed with the alpha.6 script. Formatting,
  Clippy, app JavaScript syntax and static website build checks passed.

This release adds a terminal menu and short commands. It does not add a new USB
backend, change native driver cleanup, or establish physical USB/Bluetooth
compatibility. macOS here is CLI/control-plane testing only; USB import/export
was still unavailable in alpha.6. See the [terminal guide](terminal.md) and the separate Mac development preview below.

## macOS export development preview (2026-09-09)

The Mac source now includes native read-only inventory, a per-device IOKit
export worker, authenticated transport integration, distinct export/import
capabilities in API/UI/CLI, and a normal-user .app/LaunchAgent build.
[ADR 0004](adr-0004-macos-export.md) records the pinned upstream and constraints.

Local Apple Silicon / macOS 26 evidence:

- Seven Swift tests pass for identity/generation, conservative eligibility,
  request framing/bounds, truncated input, independent endpoint progress and
  queued cancellation. The transfer test uses a fake USB communicator.
- Nineteen Rust tests pass, including process-fixture stream echo, exclusive
  ownership, rejected-open rollback, EOF release, unexpected worker exit, and
  Mac import rejection before contacting a peer or allocating a session.
- Existing terminal acceptance passes: real-agent pairing/approval/revocation,
  pseudoterminal menu interactions and explicit HTTP device fixtures.
- Mac capability text and the disabled receiving control were checked in the
  rendered local app at phone and desktop widths; no horizontal overflow was
  observed. Device cards in this UI check were explicitly labeled fixtures.
- Developer ID app signing and strict nested-code verification passed. Native
  bundle acceptance read 11 actual USB devices, rejected an absent device,
  discovered its sibling worker, reported correct role capabilities, and shut
  down cleanly. No interface was opened during these hardware inventory checks.

No eligible adapter was attached. **Physical Mac USB export is unvalidated**,
including Mac-to-Linux and Mac-to-Windows transfer and recovery. USB receiving,
Bluetooth, composite devices and driver handoff are unavailable on Mac. The
signed distribution and CI evidence below do not establish device compatibility.

## Mac preview distribution (2026-09-09)

- Developer ID signing, Apple notarization, ticket stapling, strict bundle
  verification and Gatekeeper acceptance passed on the development Mac.
  The initial accepted submission was `eef61bfa-627f-40df-b53e-fae1e7d043ac`;
  the Swift compatibility rebuild at `9a4153c` was also accepted as
  `543b2a7e-4012-4014-854f-b8b5b1e6147b`.
- The notarized app was installed in the user's Applications folder. Opening
  its bundled launcher starts a user LaunchAgent. The installed process reports
  a ready worker, export enabled, import disabled, and 11 real devices. No device
  was shared by this installation check; first-run Finder dialogs were not
  observed as a complete fresh-user installation flow.
- Source CI uses macOS 14 / Xcode 16.2 (Swift 6.0) and Rust 1.97. The initial CI
  failures exposed the runner's older default Swift and an upstream trailing
  parameter comma. The pinned patch now accepts Swift 6.0. The local notarized
  build uses macOS 26 / Swift 6.3.2 / Rust 1.98.1. These are separate builds;
  a CI package is ad-hoc signed, not a public release artifact.
- The Mac download panel was rendered at desktop and phone widths, including
  1440, 390 and 320 px overflow checks. It explains the three installation steps,
  limited export scope and unavailable USB receiving/Bluetooth roles.
- The [Mac preview release](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-macos-preview.1)
  records the final source revision, SHA-256, notarization submission and CI
  results. The general Windows/Linux release remains alpha.6.

## Still unvalidated or unavailable

- Physical USB serial, printers, dedicated Bluetooth adapters, and peripheral
  pairing. No physical-device support matrix is claimed.
- Bluetooth peripheral use, BLE profile forwarding, audio, gamepads, and built-in
  controller migration. Whole USB-adapter sharing code is not Bluetooth proof.
- Physical storage/input/network hardware and these new classes on Windows.
  Alpha.4 permits conditional handoff and has Linux virtual-fixture results below.
  Hub groups share child devices; hub hardware is never exported. Audio/video-only
  classes remain disabled.
- Windows-to-Windows and physical macOS export; macOS import is unavailable. ARM64/ARMv7 packages now have
  separate installation evidence; physical Raspberry Pi USB remains unvalidated.
- Physical hotplug, host suspend/resume, sustained workloads, packet-loss tests,
  power loss, and additional kernel recovery behavior.
- Automatic discovery/reconnect/address updates, a hosted production relay,
  signed Windows installers, auto-updates, and a fully observed zero-terminal installation
  experience. Debian/Ubuntu packages now automate dependencies and USB setup.

## Reproduce safely

Use a disposable Linux VM for the exporter. `tests/kernel-gadget.sh start`
creates only the named virtual fixture; `stop` removes it. The guest requires
configfs, `dummy_hcd`, distribution USB/IP tools, and the PortRelay helper.
Never use a production input, network, or storage device as the fixture.

`tests/two-host-smoke.py --help` documents the two SSH hosts, agent state path,
and optional VM UDP forward. It exercises transfer, disconnect, repeated
connections, and trust revocation. Its defaults use `/run/portrelay` on the
exporter and `/run/portrelay-test` on the importer. Invites/API credentials stay
in process memory and are not printed.

`tests/lifecycle-smoke.py --help` requires explicit test-host confirmation and
service names. It deliberately kills the named helper and agent processes.
Run it only after configuring relay-only agents in the isolated fixture setup.
