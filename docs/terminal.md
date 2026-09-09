# Use PortRelay from a terminal

Requires **alpha.6 or newer**, or a build of the current source. Run commands as
the regular account that owns the installed application, without sudo.
Linux, including Raspberry Pi OS Lite, can use this entire flow through SSH.
Windows uses the same commands with its installed executable.

## Open the menu

On Linux, install using the [headless guide](raspberry-pi.md), then run:

```sh
portrelay check
portrelay menu
```

On Windows, open PortRelay once and enable USB support. In PowerShell:

```powershell
& "$env:ProgramFiles\PortRelay\portrelay.exe" menu
```

Choose a numbered item and press Enter. Use **0**, **q**, or an empty answer to
cancel a choice. The menu does not need a browser, mouse, or desktop session.
Closing the menu or SSH connection leaves the installed headless agent running.

1. **Add your computer.** On the device owner, choose **Create invitation**.
   Copy the entire invitation to **Add computer** on the receiver. Back on the
   owner, choose **Approve computer** and select the expected requester.
2. **Share a device.** On the owner, choose **Share local device**. Select the
   device and the approved computer. Prepare any disk, input, network, or
   Bluetooth adapter as instructed, then explicitly accept its warnings.
3. **Use and return it.** On the receiver, choose **Connect remote device**,
   then the owner and device. **Connections** shows actual attachment and
   recovery state. Choose **Disconnect device** when finished. Eject disks on
   the receiver first.

The other computer can use the normal application window instead of this menu.
Invitations expire after ten minutes and work once. Pairing, owner approval,
and per-device sharing are separate steps. A locally approved computer may
still be offline or waiting for approval on the other side.

## Short commands

Commands with missing selections prompt in a terminal. In scripts they fail
with instructions instead of guessing. Select a computer by its exact name,
full ID, or a unique ID prefix of at least eight characters. Duplicate names
require an ID. Device selections accept exact names or IDs.

```sh
portrelay devices
portrelay computers
portrelay invite --plain
portrelay pair
portrelay approve "Laptop"
portrelay share 1-2 --to "Laptop"
portrelay remote "Pi"
portrelay connect "Pi" --device 1-2
portrelay sessions
portrelay disconnect SESSION_ID
portrelay unshare 1-2
portrelay revoke "Laptop"
```

`Laptop`, `Pi`, `1-2`, and `SESSION_ID` are examples; use your listed values.
`portrelay pair` accepts a pasted invitation followed by Enter in a terminal,
or an invitation piped through stdin. Keep invitation values out of shell
history and public logs. `portrelay rename "Office Pi"` changes the name used
by new invitations.

For Windows commands in this PowerShell session, first define:

```powershell
Set-Alias portrelay "$env:ProgramFiles\PortRelay\portrelay.exe"
```

For unattended scripts, risk-bearing devices require explicit flags after
preparing the device according to the [handoff guide](device-sharing.md):

```sh
portrelay share 1-2 --to "Laptop" --acknowledge storage
portrelay disconnect SESSION_ID --ejected
```

The acknowledgement values are `input`, `storage`, `network`, and `bluetooth`.
Composite devices may need several values, separated by commas. `--ejected`
confirms that every affected disk has already been ejected on the receiving
computer; it does not eject disks for you. It also applies to `unshare` and
`revoke` when they would close disk connections. Mounted disks, active network
adapters, and other blocked devices cannot be made shareable with these flags.

A device that changes after selection must be selected again. Sharing a hub
child grants only that device, not future devices plugged into the hub. The
terminal menu shares children individually; the desktop window also has a group
control. Bluetooth remains whole USB-adapter sharing, with pairing performed
through the receiver's Bluetooth tools after attachment.

## Automation and troubleshooting

`portrelay status`, `portrelay invite` without `--plain`, and `portrelay api`
retain their JSON output. Piped `portrelay pair` also retains JSON output.
`portrelay api` accepts the existing typed JSON actions for advanced automation.
Use `--help` on a command for its arguments.

`portrelay check` verifies the agent and USB helper; it does not prove that a
physical peripheral works. The menu shows the current helper status and
`portrelay sessions` includes recent connection errors. The authenticated
control API stays on loopback. This feature does not add automatic peer
discovery, a public relay, macOS USB support, or new physical-device evidence.
