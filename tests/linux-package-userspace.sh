#!/bin/sh
# Run in a fresh, disposable Debian/Ubuntu container. No USB or host mounts.
set -eu
[ "${PORTRELAY_DISPOSABLE_CONTAINER:-}" = 1 ] || exit 1
[ "$(id -u)" -eq 0 ] || exit 1
package=$1
apt-get update
apt-get install --no-install-recommends -y "$package" python3
[ "$(dpkg-query -W -f='${Architecture}' portrelay)" = "$(dpkg --print-architecture)" ]
portrelay --version
ldd /usr/lib/portrelay/portrelay
useradd -m -s /bin/sh portrelay-test
work=$(mktemp -d)
worker=''
cleanup() {
    if [ -n "$worker" ]; then kill "$worker" 2>/dev/null || true; wait "$worker" || true; fi
    rm -rf "$work"
}
trap cleanup EXIT
runuser -u portrelay-test -- portrelay run --no-open > "$work/agent.log" 2>&1 &
worker=$!
attempts=0
until runuser -u portrelay-test -- portrelay status > "$work/state.json" 2>/dev/null; do
    attempts=$((attempts + 1))
    if [ "$attempts" -ge 30 ]; then cat "$work/agent.log"; exit 1; fi
    sleep 1
done
python3 - "$work/state.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
assert not s['helper_ready']
assert not s['sessions']
PY
runuser -u portrelay-test -- portrelay invite > "$work/invite.json"
python3 - "$work/invite.json" <<'PY'
import json, sys
json.load(open(sys.argv[1]))
PY
kill "$worker"
wait "$worker" || true
worker=''
apt-get install --reinstall --no-install-recommends -y "$package"
apt-get remove -y portrelay
[ ! -e /usr/lib/portrelay/portrelay ]
apt-get purge -y portrelay
echo 'PASS: architecture, dynamic libraries, non-root agent, status, invitation, clean exit, reinstall, remove, purge (userspace only).'
