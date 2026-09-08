# Device handoff (alpha.4)

PortRelay lends a whole USB device to one approved computer at a time. The
owner stops using that device while it is connected remotely. Disconnect returns
it to the owner. Both computers need the receiving/sending OS drivers.

## Choose what to share

| Device | Before sharing | On the receiving computer |
| --- | --- | --- |
| Keyboard or mouse | Keep another way to control the owner. Accept the input-device warning. | Use the normal keyboard/mouse driver. |
| USB disk | Linux: unmount **every** volume; disable swap and remove storage-pool users. Windows: take the disk **offline** in Disk Management. Boot/system disks stay local. | Mount/use it normally. Eject or unmount every volume before disconnecting. |
| USB network adapter | Disable that adapter in the owner's network settings. Use a different connection for PortRelay. | Enable/configure it with the normal network settings. |
| USB hub | Choose its connected devices, individually or with **Share available**. | Connect the selected devices individually. The hub hardware stays with the owner. |
| Dedicated USB Bluetooth adapter | Keep another input/network connection. Accept the Bluetooth warning. | Connect, then choose **Open Bluetooth settings** to pair a peripheral. |

Hub sharing is a snapshot: devices plugged in later stay private. Each child has
its own permission and exclusive session. Blocked children are shown with the
reason and are skipped by the group action. Windows groups use the parent
reported by Plug and Play; Linux uses the physical USB port tree. Root controllers
are not shareable hubs.

The source checks disk/network usage again immediately before export. An unknown
usage state blocks handoff. Windows may need another inventory refresh while its
disk/network providers start; the device stays blocked until usage is verified. Composite devices carry **all** applicable warnings.
Imported devices cannot be re-exported. A disk whose Windows driver cannot take
it offline remains blocked; this includes some removable flash drives.

## Bluetooth, step by step

1. Plug a separate USB Bluetooth adapter into the computer near the peripheral.
2. Share the adapter with your approved receiving computer.
3. On the receiving computer, select the adapter and choose **Connect**.
4. Put the peripheral in pairing mode. Choose **Open Bluetooth settings**, select
   the borrowed adapter if your OS offers multiple controllers, and pair there.
5. Disconnect in PortRelay when finished. Local Bluetooth connections may need
   to reconnect after the adapter returns.

The radio stays beside the original computer, even over the internet. This path
transports the whole USB adapter, not individual BLE services or Bluetooth audio
profiles. A peripheral connected to a remote adapter still depends on the OS
Bluetooth stack and that adapter's driver. Built-in PCI/UART Bluetooth controllers
cannot use USB/IP. Physical pairing and adapter compatibility still need testing;
the settings button is not evidence that every adapter or profile works.

## Limits to keep in mind

- A network failure is like unplugging the device. In-flight disk writes can be
  lost. Eject before ending a loan, keep backups, and do not use this alpha for
  the only copy of important data. The app does not coordinate filesystem
  ownership with unrelated raw-disk tools or other mount namespaces.
- A blocked device is not made safe by accepting a warning. Release its local
  use and refresh. The application does not unmount disks or disable adapters
  behind your back.
- Bluetooth audio, webcams and timing-sensitive input need device-specific
  latency/isochronous validation. No universal compatibility claim is made.
- macOS USB import/export and standalone BLE bridging remain unimplemented.
- Windows' previously recorded controller-removal/shutdown issue remains open.

See [the validation record](validation.md) for executed tests and remaining
hardware acceptance gates. Earlier alpha.3 blanket class restrictions are
superseded by the per-device rules above; alpha.3 downloads retain their old rules.
