# Raspberry Pi and ARM Linux

## Sign in and find your computers

Open PortRelay and choose **Sign in with GitHub** on each computer. They appear
under **Your computers** automatically. With SSH only, use
`portrelay login --no-open` and open the link on your laptop.
[Account setup and removal](accounts.md). The manual pairing commands below are
an optional offline alternative.

PortRelay alpha.6 provides ARM64 and ARMv7 packages, including a setup mode for
Raspberry Pi OS Lite and machines used only through SSH. Physical Raspberry Pi
USB/Bluetooth compatibility remains unvalidated; see the [test record](validation.md).

## Install over SSH

Log in as your normal user and run:

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh | sh -s -- --headless
```

Enter your sudo password if asked. The installer selects the operating system
and architecture, checks the pinned package SHA-256, installs dependencies,
enables the USB helper, and starts the agent without a browser. This explicitly
enables systemd lingering for your account so it also starts at boot and stays
running after you disconnect SSH. No device is shared automatically.

Check readiness and open the selection menu:

```sh
portrelay check
portrelay menu
```

Use these commands as the same normal user, not with sudo. Choose **Create
invitation** on the owner and **Add computer** on the receiver, then approve the
request on the owner. Choose **Share local device** on the owner and **Connect
remote device** on the receiver. The [terminal guide](terminal.md) also covers
short commands and scripts. The other computer can use its normal app window.

If you log in directly as root, choose an **existing non-root account**:

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh | sh -s -- --headless --user YOUR_USERNAME
```

Already installed? Use `sudo /usr/lib/portrelay/setup-headless "$USER"` from the
owning regular account. On a Pi with a desktop, omit `--headless` and open
PortRelay from the applications menu instead.

## Choose an image and package

| Operating system | Package architectures |
| --- | --- |
| Raspberry Pi OS Bookworm (12) / Trixie (13), including Lite | ARM64, ARMv7 (`armhf`) |
| Debian 12 / 13 | AMD64, ARM64, ARMv7 (`armhf`) |
| Ubuntu 24.04 | AMD64, ARM64 |

The Pi OS package is the Debian `.deb`. For a new compatible Pi installation,
64-bit Pi OS is the straightforward choice. A 32-bit OS needs the ARMv7 package,
even when its kernel reports `aarch64`. The installer uses
`dpkg --print-architecture` to avoid this mismatch.

ARMv7 packages require an ARMv7 or newer CPU. **Pi 1 and the original Pi Zero /
Zero W use ARMv6 and are excluded.** Zero 2 W is a different model. Consult
Raspberry Pi's [official image/model list](https://www.raspberrypi.com/software/operating-systems/)
when choosing an OS. An architecture match is not a physical-device test result.

[Download packages and checksums](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.6).
The advanced tarballs are named `linux-aarch64` and `linux-armv7`; prefer the
`.deb` installer unless you intend to manage dependencies and services manually.
Other NAS/router distributions, Alpine, ARMv6, and Windows ARM64 are not targets.

## USB and Bluetooth on a Pi

Use a kernel containing `usbip-host` and `vhci-hcd`. The standard Raspberry Pi
[64-bit kernel configuration](https://github.com/raspberrypi/linux/blob/rpi-6.18.y/arch/arm64/configs/bcm2711_defconfig)
includes both. Setup checks the modules on the **running** kernel before reporting
ready. If modules are missing after a kernel update, reboot and retry setup.
PortRelay does not replace your kernel or start a public `usbipd` server.

Do not share the Pi's boot disk or the network interface carrying your SSH
connection. Mounted/in-use storage and active USB network adapters remain
blocked by the normal usage checks. Input devices need explicit handoff consent.
The USB loan is exclusive; disconnect it to return it to the Pi.

Bluetooth currently means a **dedicated USB Bluetooth dongle** using that same
loan path. The Pi's built-in Bluetooth radio is not a USB dongle and is not
supported by this feature. Physical Bluetooth pairing still needs testing.

## Stop, update, or remove

`systemctl --user stop portrelay` stops your agent and returns its devices.
`systemctl --user start portrelay` starts it again. Re-run the installer to install
its pinned release; it is not an automatic updater. Disconnect borrowed storage
cleanly before any update or shutdown.

Remove with `sudo apt remove portrelay`, or `sudo apt purge portrelay` to also
remove the system owner configuration. Your private identity/pairings remain.
Lingering is an account-wide systemd setting, so uninstall does not disable it
for your other services. If you no longer need any services without a login,
explicitly run `sudo loginctl disable-linger "$USER"`.

Agent logs: `journalctl --user -u portrelay`. Helper logs:
`sudo journalctl -u portrelay-helper`. Kernel-tool installation details:
`sudo cat /var/lib/portrelay/setup.log`.
