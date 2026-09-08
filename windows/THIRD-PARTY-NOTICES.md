# Windows distribution notices

PortRelay's Rust agent and desktop launcher are MIT licensed. Its separate
Windows device service is a modified usbipd-win program, GPL-3.0-only.
The complete modified service source is supplied beside every Windows installer
as `portrelay-VERSION-windows-backend-source.zip`.

- usbipd-win 5.3.0, Frans van Dorsselaer and contributors:
  https://github.com/dorssel/usbipd-win/tree/aa3db8b82c4cb5071fd31bc54211606c70886912
  GPL-3.0-only. Changes: private named-pipe transport, owner-authenticated control
  service, conservative device inventory, native import bridge, recovery journals.
  The original COPYING.md and LICENSES directory are included with the service.
- Oracle VirtualBox USB drivers 7.2.2: GPL-3.0-only. The original driver files,
  copyright/license sidecars, and source offer are in `device-service/Drivers`.
  Corresponding Oracle source is freely available at
  https://download.virtualbox.org/virtualbox/7.2.2/VirtualBox-7.2.2.tar.bz2 .
  This free, noncommercial redistribution preserves the upstream GPL section 6c offer.
- usbip-win2 0.9.8.0, Vadym Hrynchyshyn and contributors: BSD-2-Clause.
  https://github.com/vadimgrn/usbip-win2/tree/83bd1f781d57ed6efdf15530c55710cf5d4482bc
  The original signed installer retains its license and third-party notices.
  No kernel driver has been modified or self-signed by PortRelay.
- Microsoft .NET 9 runtime: MIT, with component notices from the official .NET
  distribution. https://github.com/dotnet/runtime/blob/v9.0.19/LICENSE.TXT
  https://github.com/dotnet/runtime/blob/v9.0.19/THIRD-PARTY-NOTICES.TXT

Driver installation does not grant PortRelay permission to redistribute devices
or their firmware. Device compatibility must be tested separately.
