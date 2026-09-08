#!/bin/sh
set -eu
[ "$(id -u)" -eq 0 ] || { echo 'Run with sudo.' >&2; exit 1; }
systemctl stop portrelay-agent.service portrelay-helper.service
for record in /run/portrelay/*.json; do
    [ ! -e "$record" ] || { echo 'A device still needs recovery. Restart the helper and resolve it before uninstalling.' >&2; exit 1; }
done
systemctl disable portrelay-agent.service portrelay-helper.service
rm -f /etc/systemd/system/portrelay-agent.service /etc/systemd/system/portrelay-helper.service
rm -f /usr/local/bin/portrelay /usr/local/lib/portrelay/portrelay /usr/local/share/applications/portrelay.desktop
rmdir /usr/local/lib/portrelay
systemctl daemon-reload
echo 'PortRelay removed. Your identity and pairing records were kept in your user data directory.'
