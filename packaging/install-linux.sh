#!/bin/sh
# Install the root helper and an unprivileged agent for one local user.
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run with sudo: sudo ./install-linux.sh YOUR_USERNAME' >&2; exit 1; }
target_user=${1:-${SUDO_USER:-}}
[ -n "$target_user" ] || { echo 'Specify the desktop username.' >&2; exit 1; }
target_uid=$(id -u "$target_user")
[ "$target_uid" -ne 0 ] || { echo 'The application must run as a regular user.' >&2; exit 1; }
case "$target_user" in *[!a-zA-Z0-9_-]*) echo 'Unsupported username.' >&2; exit 1;; esac
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
binary=${2:-$script_dir/portrelay}
[ -x "$binary" ] || { echo 'Pass the built binary as the second argument.' >&2; exit 1; }
command -v systemctl >/dev/null
command -v runuser >/dev/null
{ [ -x /usr/bin/usbip ] || [ -x /usr/sbin/usbip ]; } || { echo 'Install USB/IP first. Ubuntu: sudo apt install linux-tools-common linux-tools-generic' >&2; exit 1; }
modinfo usbip-host >/dev/null 2>&1 && modinfo vhci-hcd >/dev/null 2>&1 || { echo 'Install the USB/IP modules for your running kernel, then retry.' >&2; exit 1; }
if systemctl is-active --quiet portrelay-agent.service; then
    echo 'PortRelay is already running. Disconnect devices and stop portrelay-agent.service before upgrading.' >&2; exit 1
fi
if [ -e /etc/systemd/system/portrelay-helper.service ] && ! grep -q -- "--uid $target_uid$" /etc/systemd/system/portrelay-helper.service; then
    echo 'An existing helper belongs to another user. Uninstall it first.' >&2; exit 1
fi
install -d -m 755 /usr/local/lib/portrelay
install -m 755 "$binary" /usr/local/lib/portrelay/portrelay.new
mv /usr/local/lib/portrelay/portrelay.new /usr/local/lib/portrelay/portrelay
ln -sfn /usr/local/lib/portrelay/portrelay /usr/local/bin/portrelay
cat > /etc/systemd/system/portrelay-helper.service <<UNIT
[Unit]
Description=PortRelay restricted USB helper
After=systemd-udevd.service
[Service]
ExecStart=/usr/local/lib/portrelay/portrelay helper --uid $target_uid
Restart=on-failure
RestartSec=3
TimeoutStopSec=30
RuntimeDirectory=portrelay
RuntimeDirectoryMode=0711
RuntimeDirectoryPreserve=yes
NoNewPrivileges=yes
PrivateTmp=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths=/sys/bus /sys/devices /run/portrelay
RestrictAddressFamilies=AF_UNIX AF_INET AF_NETLINK
IPAddressDeny=any
IPAddressAllow=localhost
[Install]
WantedBy=multi-user.target
UNIT
cat > /etc/systemd/system/portrelay-agent.service <<UNIT
[Unit]
Description=PortRelay device sharing application
After=network.target portrelay-helper.service
Wants=portrelay-helper.service
[Service]
User=$target_user
ExecStart=/usr/local/lib/portrelay/portrelay run --no-open
Restart=on-failure
RestartSec=3
TimeoutStopSec=30
NoNewPrivileges=yes
[Install]
WantedBy=multi-user.target
UNIT
install -d -m 755 /usr/local/share/applications
cat > /usr/local/share/applications/portrelay.desktop <<'DESKTOP'
[Desktop Entry]
Name=PortRelay
Comment=Share USB devices between your computers
Exec=/usr/local/bin/portrelay
Terminal=false
Type=Application
Categories=Network;Utility;
StartupNotify=false
DESKTOP
systemctl daemon-reload
systemctl enable --now portrelay-helper.service portrelay-agent.service
systemctl is-active --quiet portrelay-helper.service
systemctl is-active --quiet portrelay-agent.service
attempts=0
until runuser -u "$target_user" -- /usr/local/bin/portrelay check >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    [ "$attempts" -lt 20 ] || { echo 'Services were installed but did not become ready. Inspect journalctl -u portrelay-agent -u portrelay-helper.' >&2; exit 1; }
    sleep 1
done
echo 'Installed. Open PortRelay from your applications menu, or run: portrelay'
echo 'On a firewall-protected LAN, allow UDP 24816 only from computers you intend to pair.'
