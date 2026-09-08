# Windows device service

This directory is GPL-3.0-only. It extends the pinned usbipd-win 5.3.0
source as a separate process; the PortRelay agent remains MIT licensed.
`prepare.py` applies the small transport changes and adds the service files.
No upstream TCP server is shipped or started. The USB/IP export transport is
a LocalSystem-only named pipe. The control pipe accepts the installed owner
and LocalSystem, rejects remote clients, and checks process identities.

The native import driver is the signed usbip-win2 0.9.8.0 release (BSD-2-Clause).
Its per-session loopback connection must belong to the Windows kernel (PID 4)
before receiving any USB/IP data. Connection retry is disabled.

Build inputs and corresponding source are packaged by `../build.ps1`.
The Oracle driver binaries and their upstream license/source offer are preserved.
This work does not modify, self-sign, or relax verification of kernel drivers.
