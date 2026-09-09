#!/bin/sh
# Fresh, disposable systemd CI runners only. Never shares or binds a USB device.
set -eu
[ "${GITHUB_ACTIONS:-}" = true ] || { echo 'Run this test only on a disposable GitHub Actions runner.' >&2; exit 1; }
[ "$(id -u)" -eq 0 ] || exit 1
[ "$#" -eq 1 ] || exit 1
package=$(realpath "$1")
account=portrelay-ci
if id "$account" >/dev/null 2>&1; then echo 'Test account already exists.' >&2; exit 1; fi
apt-get update
apt-get install --no-install-recommends -y "$package"
useradd -m -s /bin/bash "$account"
uid=$(id -u "$account")
as_owner() { runuser -u "$account" -- "$@"; }
user_systemctl() { as_owner env XDG_RUNTIME_DIR="/run/user/$uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" systemctl --user "$@"; }
if /usr/lib/portrelay/setup-headless root; then echo 'Root ownership was accepted.' >&2; exit 1; fi
/usr/lib/portrelay/setup-headless "$account"
/usr/lib/portrelay/setup-headless "$account"
as_owner portrelay check >/dev/null
as_owner portrelay invite > /tmp/portrelay-ci-invite.json
python3 -c 'import json; json.load(open("/tmp/portrelay-ci-invite.json"))'
as_owner portrelay status > /tmp/portrelay-ci-state.json
python3 -c 'import json; s=json.load(open("/tmp/portrelay-ci-state.json")); assert s["helper_ready"]; assert not s["sessions"]'
[ "$(loginctl show-user "$account" -p Linger --value)" = yes ]
pid=$(user_systemctl show portrelay -p MainPID --value)
[ "$(ps -o uid= -p "$pid" | tr -d ' ')" = "$uid" ]
user_systemctl is-enabled portrelay.service
systemctl is-enabled portrelay-helper.service
# Restart the user manager without any desktop or login session.
systemctl stop "user@$uid.service"
systemctl start "user@$uid.service"
as_owner env XDG_RUNTIME_DIR="/run/user/$uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" portrelay desktop --no-open
as_owner portrelay check >/dev/null
apt-get install --reinstall --no-install-recommends -y "$package"
as_owner portrelay check >/dev/null
apt-get remove -y portrelay
if user_systemctl is-active --quiet portrelay.service; then exit 1; fi
if systemctl is-active --quiet portrelay-helper.service; then exit 1; fi
apt-get purge -y portrelay
[ ! -e /etc/portrelay/owner.conf ]
loginctl disable-linger "$account"
systemctl stop "user@$uid.service"
userdel -r "$account"
rm /tmp/portrelay-ci-invite.json /tmp/portrelay-ci-state.json
echo 'PASS: fresh install, root rejection, headless setup, repeat setup, non-root agent, helper readiness, no-login restart, reinstall, remove, purge.'
