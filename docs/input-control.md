# Keyboard and mouse control

PortRelay can send keyboard, mouse buttons, movement and scrolling from its
local app window to an **unlocked Linux desktop**. This is a separate feature
from sharing a whole USB device. Your mouse stays attached to the sending Mac.

Install **alpha.9 or Mac preview.4** for the current dashboard. Input control
was introduced in alpha.7; account discovery was introduced in alpha.8. Use a desktop browser with
Pointer Lock support, such as Chrome. Mac to ARM Linux has passed the kernel
input acceptance test. Other sending platforms and Linux desktop combinations
need their own interactive acceptance runs.

## Set up once

1. Install PortRelay on both computers and sign in with the same GitHub account.
   They appear under **Computers** automatically.
2. On Linux, open **Keyboard & mouse → Enable receiving control**. Approve the administrator
   prompt. This installs a restricted virtual keyboard and mouse service;
   USB/IP support is not needed for this feature.
3. On Linux, open **Computers → Manage** for your Mac and choose
   **Allow keyboard & mouse**.
   Pairing alone does not grant control.

If you manage the Linux computer through SSH, the equivalent is:

```sh
portrelay desktop --no-open
sudo /usr/lib/portrelay/setup-input
portrelay input-allow 'My Mac'
```

The Linux user still needs a local desktop session, signed in and unlocked.
The current receiver supports the active user on `seat0`; it cannot control a
login screen, another user's session, or a server without a desktop. USB
sharing and the terminal menu continue to work on headless systems.

## Use it

1. Keep the Linux computer's screen visible, on its monitor or through your
   existing video viewer.
2. On Mac, open **Keyboard & mouse**, choose the Linux computer, then
   **Control [computer name]**. Keep the PortRelay tab in
   front and allow mouse control if the browser asks.
3. Move, click, scroll or type. Press **Esc** to return to the Mac.

Switching windows, hiding/closing the tab, stopping control on the receiver,
locking the Linux desktop, or losing the connection ends control. There is
no automatic reconnect. Click Control again when both sides are ready.

To remove permission on Linux, choose **Stop allowing control**, or run:

```sh
portrelay input-deny 'My Mac'
```

## Current limits

- Input only: no screen video, clipboard, file transfer or automatic screen-edge
  switching. The remote desktop's keyboard layout determines typed characters.
- Escape always returns to the sender. Some browser/OS shortcuts remain local;
  this is not a global keyboard grab. Text composition/IME and media keys are
  not forwarded. Physical key positions, standard buttons and relative motion
  are forwarded; the USB identity and vendor software are not.
- Receiving control currently requires Linux. Mac/Windows receiving, whole
  Mac USB HID export and Bluetooth adapter sharing are separate unfinished work.
- Ordinary browser delivery, network delay and the receiver's pointer settings
  affect responsiveness. This preview does not promise gaming latency.

## Implementation and cleanup

The focused browser sends bounded event batches to its authenticated local
agent. The agent uses the existing authenticated, encrypted iroh connection.
On Linux a separate root helper writes to two `uinput` virtual devices. It
accepts only the configured local UID over a mode-0600 Unix socket, checks
desktop ownership, bounds/rate-limits events, and permits one session at a time.

The browser sends a heartbeat every 200 ms. Network/helper reads expire after
two seconds without a batch; cleanup scheduling and desktop checks can add
time. Explicit stop closes the stream immediately. Dropping the helper's
virtual-device handles removes the devices, including after helper termination.
No typed text or input-event payloads are logged by the application.

The opt-in `tests/input-smoke.py` test uses two isolated agents and the separate
`portrelay-input-test` helper. Its capture fixture grabs only the new PortRelay
virtual devices so test input cannot reach other desktop applications. This
proves the kernel input path, not visible application behavior or every keyboard.

See [design decision](adr-0005-input-control.md) and [validation](validation.md).
