# PortRelay Windows service: corresponding source

This archive contains the complete modified usbipd-win 5.3.0 service source,
its original license notices, pinned NuGet dependencies, signed upstream driver
files, and PortRelay's build additions. The service is GPL-3.0-only.

Install the official .NET SDK **9.0.317**, extract this archive, and run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\REBUILD.ps1
```

The output is `rebuilt-service/`. No Git checkout, Rust installation, driver
installation, signing certificate, or security-setting change is needed to
compile this service. The archive includes the generated upstream version
constants because its original Git history is not included. Dependencies are
restored from NuGet using the committed lockfile. The .NET analyzers report
advisory style warnings; compiler failures still fail the build.

The MIT agent, desktop launcher, application UI, and full installer build are
in the matching PortRelay release source at
https://github.com/cobanov/portrelay . Follow `windows/build.ps1` there to build
the complete application. `PortRelay-build/` in this archive records those
Windows additions, but is not a second standalone application checkout.

The unchanged Oracle driver license/source offers are under `Drivers/`.
Oracle's corresponding driver source is available from
https://download.virtualbox.org/virtualbox/7.2.2/VirtualBox-7.2.2.tar.bz2 .
The separate original usbip-win2 client installer and its BSD notices are
distributed with the full application; its source is at
https://github.com/vadimgrn/usbip-win2/tree/83bd1f781d57ed6efdf15530c55710cf5d4482bc .
