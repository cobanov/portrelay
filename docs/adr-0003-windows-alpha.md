# ADR 0003: native Windows USB sharing

Date: 2026-09-08. Scope: Windows 11 x64. Linux remains supported by its existing
kernel backend. macOS native USB is a separate, unresolved integration.

## Decision

Reuse two established backends instead of writing or self-signing kernel code:

- Export: usbipd-win 5.3.0, commit
  `aa3db8b82c4cb5071fd31bc54211606c70886912`, with its original Oracle 7.2.2
  signed drivers. PortRelay supplies a modified GPL service as a separate process.
- Import: the original signed usbip-win2 0.9.8.0 x64 installer, source commit
  `83bd1f781d57ed6efdf15530c55710cf5d4482bc`. This attaches a device to native
  Windows, without WSL. The installer SHA-256 is
  `81f426741f7ee2ed991febe24a22daca8400b6ae2f171054e3fb404897e15d39`.

The MIT Rust agent and embedded UI use the existing encrypted peer protocol.
They run as the desktop user. A LocalSystem Windows service performs device
operations. One installer includes the app, self-contained .NET service,
driver installers, notices, and uninstall scripts. Enabling USB support once
uses the normal Windows administrator prompt.

## Local boundaries

The original usbipd-win MSI is not used. Its public TCP server and firewall rule
are not installed. The modified export transport is a LocalSystem-only named
pipe. The control pipe admits only the configured user and LocalSystem and
rejects remote pipe clients. The service checks the connecting process token.
The agent compares the pipe server PID with the running, LocalSystem service
registered in Windows Service Control Manager. Reading a SYSTEM process token
from an ordinary desktop account is not a valid authentication implementation.

The signed import driver requires TCP. For each authorized lease, the service
opens one ephemeral IPv4 loopback listener. It checks the exact accepted TCP
tuple in the Windows owner-PID table and accepts only a kernel connection.
Loopback alone does not authenticate a caller.

The upstream virtual controller normally allows ordinary local users to open
it. PortRelay requires a dedicated installation and restricts its device security
descriptor to SYSTEM and administrators, using the supported Windows device
installation API. Restarting that controller applies the restriction and closes
old handles. The service verifies the **live device object's** owner and DACL
before reporting ready or attaching a device. Another standard user therefore
cannot ask this driver to impersonate the authorized kernel connection.

References: Microsoft's [device registry security API](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/setting-device-object-registry-properties-after-installation)
and [KMDF device security overrides](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdfdevice/nf-wdfdevice-wdfdeviceinitassignsddlstring).
No INF, catalog, or kernel binary is edited; Secure Boot and signature enforcement
stay enabled. Administrators and SYSTEM remain trusted local principals.

The only application firewall rule admits the agent's encrypted UDP traffic.
The local HTTP control API stays on loopback and requires its private bearer
token. No raw USB/IP TCP port is made reachable through the network.

## Ownership and failure recovery

Existing peer approval, per-device grants, exclusive leases, invitation expiry,
and revocation apply to Windows. Device identity includes the Windows instance,
arrival identity, and service lifetime. Replaced or unverifiable devices require
new permission. Imported devices and protected input, storage, hub, and network
classes cannot be exported through the UI or device service.

The privileged service journals every export/import before changing a device.
Exports record the original Windows instance. Imports record the exact
per-session loopback URL and assigned virtual port. Cleanup closes the stream,
waits for upstream release, restores the original driver, and removes only its
own virtual attachment. Failed cleanup retains its journal and disables new
device operations until recovery succeeds. Service startup reconciles journals;
Windows restarts the service after an unexpected termination.

Upgrade and uninstall stop this installation's agent, wait for device cleanup,
and reject unresolved recovery. The installer does not replace an independently
installed usbipd-win, VirtualBox USB monitor, or USBip client.

## Distribution and limits

The device service is GPL-3.0-only and shipped with complete corresponding
modified source and a standalone rebuild script. The agent remains a separate
MIT program. Upstream Oracle source offers and BSD/.NET/Rust notices accompany
the installer. See [Windows notices](../windows/THIRD-PARTY-NOTICES.md).

The first package targets native x64 Windows 11. The architecture restriction
uses `x64os` because driver binaries cannot run through ARM64 x64 emulation.
See [Inno Setup's driver architecture guidance](https://jrsoftware.org/ishelp/topic_setup_architecturesallowed.htm).
The PortRelay wrapper itself is unsigned; upstream kernel signatures are a
different property. No stable, universal device-compatibility claim follows
from compilation or signed drivers. Actual results belong in the
[validation record](validation.md).
