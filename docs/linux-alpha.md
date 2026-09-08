# Set up PortRelay

The Linux alpha shares USB devices with Windows or other Linux computers. Start with a
test device: physical USB and Bluetooth compatibility is still unvalidated.
The current packages are for Intel/AMD 64-bit Linux with systemd.

## One-command install

On **Ubuntu 24.04 or Debian 13, Intel/AMD 64-bit**, paste this in Terminal:

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh | sh
```

The script selects your distribution, verifies the alpha.4 package against a
pinned SHA-256, and uses `apt-get` to install it and its dependencies. Enter your
administrator password if asked. Then open **PortRelay** from your applications
and follow steps 2 and 3 below. It does not enable USB or share devices for you.
Unsupported distributions and architectures stop before downloading a package.

[Read the script](../web/install.sh). To inspect it before running:

```sh
curl -fsSL https://portrelay.cobanov.dev/install.sh -o install-portrelay.sh
less install-portrelay.sh
sh install-portrelay.sh
```

Re-running installs the same pinned release; this is not an automatic update
service. `sh install-portrelay.sh --download-only` checks the package without
installing and removes the temporary download afterward.

## Three steps on each computer

1. **Install the package.** Download for
   [Ubuntu](https://github.com/cobanov/portrelay/releases/download/v0.1.0-alpha.4/portrelay-0.1.0-alpha.4-ubuntu-amd64.deb) or
   [Debian](https://github.com/cobanov/portrelay/releases/download/v0.1.0-alpha.4/portrelay-0.1.0-alpha.4-debian-amd64.deb).
   Open the file with your system's software installer and choose **Install**.
2. **Open PortRelay.** Find it in your applications. Give this computer a name,
   then choose **Enable USB sharing** and approve the system password prompt.
   PortRelay prepares USB support for you. Ubuntu may download matching kernel
   tools, so keep the computer online during this step.
3. **Add your other computer.** Choose **Create & copy invitation** on one,
   then **I have an invitation** on the other. Paste it and choose **Add computer**.
   Approve the request on the first computer. You can also save and open an
   invitation file. Invitations are private, single-use, and expire in ten minutes.

Then select the other computer and choose **Share device** beside a local USB
device. On the receiving computer, open **Use a remote device** and choose
**Connect**. **Disconnect** returns the device to its owner.

One local user owns USB setup. The app opens in your browser and runs a background
service at login. Closing the window keeps connections running. Its local window
shows actual device state; the public website is a separate concept demo.

[Release notes & checksums](https://github.com/cobanov/portrelay/releases/tag/v0.1.0-alpha.4)
· [Tested capabilities and limitations](validation.md)

<details>
<summary>If your system does not open the package</summary>

In the download folder, run the matching command:

```sh
sudo apt install ./portrelay-0.1.0-alpha.4-ubuntu-amd64.deb
# Or, on Debian:
sudo apt install ./portrelay-0.1.0-alpha.4-debian-amd64.deb
```

Use `apt install`, which resolves dependencies, rather than `dpkg -i` alone.
A desktop browser and the desktop's normal polkit permission agent are needed
for the graphical flow. Installation and authorization are tested headlessly on
Debian 13 and Ubuntu 24.04; graphical menu launch and password-dialog rendering
still need desktop observation. The binary baseline is glibc 2.36. Other distro
releases, ARM64 packages, signing, and automatic updates remain unvalidated.
Checksums detect corruption; they are not publisher signatures.

</details>

<details>
<summary>Upgrading from the earlier tarball installer</summary>

Disconnect active devices. In the old extracted package, run
`sudo ./uninstall-linux.sh`, then install the new `.deb`. The old uninstaller
preserves your computer identity and pairings. The new package refuses to
install over the old `/usr/local` services to avoid running two agents.
Subsequent `.deb` upgrades stop and restart active services automatically.

</details>

## What can be shared

Each device is private until you share it with a specific approved computer.
Only one computer can use it at a time. **Stop sharing** or removing a computer
ends its active loans. Unplugging or replacing a device invalidates its old
permission; share it again.

Alpha.4 adds input devices, unmounted disks, and disabled USB network adapters
with explicit handoff warnings. Hub groups let you share their connected devices;
the hub itself stays local. Imported devices cannot be re-exported. Storage,
keyboard/mouse and Ethernet passed virtual kernel tests on Debian → Ubuntu.
Physical devices still require their own tests. See [device handoff](device-sharing.md)
for preparation, safe disk return, and the Bluetooth pairing steps.

Bluetooth uses a **whole dedicated USB adapter**. Its radio stays beside the
original computer; existing local Bluetooth connections may stop. The app asks
for acknowledgement before sharing it. Physical Bluetooth pairing, individual
BLE services, built-in controller migration, audio, and gamepad compatibility
have not been validated or implemented as separate profiles.

## Networks

On the same LAN, no public service or account is needed. PortRelay uses encrypted
UDP 24816; a restrictive firewall may need to allow this between your computers.
Raw USB/IP port 3240 is never opened. Pairing and approval are always required.
There is no automatic discovery or address refresh yet.

Internet operation currently needs an iroh-compatible relay that you operate or
are authorized to use. It is not an automatic, hosted internet service yet.
Device content stays encrypted through the relay. The forced-relay path has
passed a kernel serial-fixture test; independent NAT behavior remains untested.

<details>
<summary>Configure an internet relay</summary>

With no active device connections, use `systemctl --user edit portrelay`:

```ini
[Service]
ExecStart=
ExecStart=/usr/lib/portrelay/portrelay run --no-open --relay https://your-relay.example
```

Run `systemctl --user restart portrelay`. Add `--relay-only` to force the relay
and disable direct IP routes. A fresh invitation may be needed after an address
change. For a manual build, use `portrelay run --relay https://your-relay.example`.
Do not start a second agent with the same data directory. Upstream development
relays are not a promised production service or free bandwidth allocation.

</details>

<details>
<summary>Headless setup and building from source</summary>

After installing the `.deb`, run as your normal user:

```sh
portrelay desktop --no-open
pkexec /usr/lib/portrelay/setup-usb
portrelay check
```

`pkexec` can use a terminal authentication agent in this manual flow. To keep
this user's service alive without a login, an administrator can explicitly enable
systemd lingering for that user. The desktop installer does not enable lingering.

For source development, use Rust 1.97.0:

```sh
cargo build --locked --release
cargo test --locked --workspace
```

The manual tarball and `packaging/install-linux.sh` remain available for developer
use and require preinstalled distribution USB/IP tools. They use the legacy
`portrelay-agent` system service; `.deb` installs use the `portrelay` user service.
Use one installation method. macOS builds the control agent but has no USB backend.

</details>

<details>
<summary>Diagnostics and uninstalling</summary>

- **Settings & status** shows capabilities and connection details.
- `portrelay status` reports devices, peers, setup, helper health, and sessions.
- `portrelay check` succeeds only when the agent and USB helper are ready.
- `portrelay open` reopens the authenticated local window.
- `journalctl --user -u portrelay` shows application logs.
- `sudo journalctl -u portrelay-helper` shows helper and recovery errors.
- Ubuntu dependency-download details are in `/var/lib/portrelay/setup.log` (root only).
- A recovery error disables further attachment. Restart `portrelay-helper` to
  retry recovery, then inspect the device. Do not delete recovery journals.
- Transport attachment does not prove application readiness; the receiving OS
  still needs an appropriate driver.
- Disconnect devices, then remove PortRelay with the system software manager or
  `sudo apt remove portrelay`. User identity and pairings are preserved.
  `sudo apt purge portrelay` also clears the system's owner configuration.

No SIP or signature-enforcement changes are required. A native
[Windows 11 x64 alpha](windows-alpha.md) is available, with Windows/Linux
virtual serial tests in both directions. macOS device integration remains
a separate future stage, with export and import evaluated independently.

</details>
