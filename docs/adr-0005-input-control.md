# ADR 0005: Focused keyboard and mouse control to Linux

Status: accepted, experimental.

## Problem

The first Mac exporter deliberately cannot seize a driver-owned USB mouse.
The immediate user need is to operate a Linux workstation from the Mac's
existing mouse and keyboard. Moving the complete USB device is not required
to deliver that interaction.

## Research and decision

- [Deskflow](https://github.com/deskflow/deskflow) already implements cross-platform
  input sharing. Its GPL-2.0 license, separate pairing/transport, desktop shell
  and global capture permissions would require a different integration and
  distribution decision. No Deskflow source is copied into PortRelay.
- Apple's [CGEvent tap API](https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate%28tap%3Aplace%3Aoptions%3Aeventsofinterest%3Acallback%3Auserinfo%3A%29)
  is a possible later native capture backend. Global capture would add OS
  permissions and a larger lifecycle surface for the first interaction.
- Browser [Pointer Lock](https://developer.mozilla.org/en-US/docs/Web/API/Pointer_Lock_API)
  supplies relative motion and an explicit user-controlled escape route in the
  existing local app window. Capture begins with a click and ends on focus loss.
- Linux [uinput](https://docs.kernel.org/input/uinput.html) creates standard
  virtual input devices without USB/IP or a kernel modification. Reuse the
  [Rust evdev crate](https://github.com/emberian/evdev), pinned to 0.13.2 and
  licensed Apache-2.0 OR MIT. Package notices include its upstream license.

Use focused browser capture, existing iroh transport, and a separate restricted
Linux input helper. Do not detach the Mac's mouse or introduce a network-facing
privileged listener. No video, clipboard or global screen-edge switching is
part of this first feature.

## Authorization and lifecycle

- Keep input-controller grants separate from approved peers and USB grants.
  Re-pairing/revocation removes them. The receiver can stop or remove permission.
- Use one input session per agent in either direction to prevent control loops.
  The root helper also has one exclusive lease across local agents.
- Accept at most 64 events per ordered batch. Bound motion, wheel and keycodes;
  exclude power, sleep and SysRq keys. Reject duplicate/out-of-sequence batches.
- Use a bounded sender queue, acknowledgements and heartbeats. Queue overload,
  timeout, stream close and process death end control rather than dropping a
  key-up and leaving it stuck. Keyboard and pointer handles are lease-scoped.
- Verify the same local UID owns the unlocked active `seat0` session before
  opening and once per second during control. No login-screen or cross-user
  control. The helper has no physical input read access.
- The local HTTP API retains loopback binding, bearer authentication, origin
  checks, CSP and body limits. Neither network nor API may choose a helper path.
  `PORTRELAY_INPUT_SOCKET` is a local process test configuration only.

## Consequences

The Mac can operate a Linux desktop without a USB-host-controller entitlement,
Accessibility or Input Monitoring for this focused mode. The browser must stay
in front; some shortcuts cannot be captured and Escape is reserved. The feature
does not imply Mac USB import, full HID export, Bluetooth virtualization or
universal OS/keyboard compatibility. Record each tested path separately.
