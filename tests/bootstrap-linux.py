#!/usr/bin/env python3
"""Installer selection and fail-closed checks; never installs a package.

Only host detection and download are fixtures. SHA-256 checking is real.
"""
import os
import hashlib
from pathlib import Path
import re
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
source = (root / "web/install.sh").read_text()

with tempfile.TemporaryDirectory(prefix="portrelay-bootstrap-test-") as directory:
    fixture = Path(directory)
    os_release = fixture / "os-release"
    installer = fixture / "install.sh"
    installer.write_text(source.replace("/etc/os-release", str(os_release)))
    marker = fixture / "download"
    env = {**os.environ, "PATH": f"{fixture}:{os.environ['PATH']}",
           "PORTRELAY_TEST_DOWNLOAD": str(marker)}

    def executable(name, text):
        path = fixture / name
        path.write_text("#!/bin/sh\n" + text)
        path.chmod(0o755)

    executable("curl", '''
url=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    https://*) url=$1 ;;
    --output)
      printf '%s' "${PORTRELAY_TEST_PAYLOAD:-corrupt package}" > "$2"
      printf '%s\\n%s\\n' "$url" "$2" > "$PORTRELAY_TEST_DOWNLOAD"
      exit 0 ;;
  esac
  shift
done
exit 1
''')
    executable("apt-get", "echo 'An installer test tried to use apt.' >&2; exit 99\n")

    def run(distro, version, arch, kernel, expected=None, error=None):
        marker.unlink(missing_ok=True)
        os_release.write_text(f"ID={distro}\nVERSION_ID={version}\n")
        executable("dpkg", f"echo {arch}\n")
        executable("uname", f'if [ "$1" = -s ]; then echo Linux; else echo {kernel}; fi\n')
        result = subprocess.run(["sh", str(installer), "--download-only"],
                                env=env, text=True, capture_output=True)
        assert result.returncode != 0, result.stdout
        if expected:
            assert "checksum mismatch" in result.stderr, result.stderr
            url, downloaded = marker.read_text().splitlines()
            assert url.endswith(expected), url
            assert not Path(downloaded).parent.exists(), "Download directory retained"
        else:
            assert error in result.stderr, result.stderr
            assert not marker.exists(), "Unsupported machine downloaded a package"

    for distro in ("debian", "raspbian"):
        for version in ("12", "13"):
            for arch, kernel in (("arm64", "aarch64"), ("armhf", "armv7l"),
                                 ("armhf", "aarch64")):
                run(distro, version, arch, kernel, expected=f"-debian-{arch}.deb")
    for version in ("12", "13"):
        run("debian", version, "amd64", "x86_64", expected="-debian-amd64.deb")
    for arch, kernel in (("amd64", "x86_64"), ("arm64", "aarch64")):
        run("ubuntu", "24.04", arch, kernel, expected=f"-ubuntu-{arch}.deb")
    run("raspbian", "13", "armhf", "armv6l", error="ARMv7 or newer")
    run("ubuntu", "24.04", "armhf", "armv7l", error="No PortRelay package")
    run("debian", "13", "riscv64", "riscv64", error="No PortRelay package")
    run("debian", "11", "arm64", "aarch64", error="Use Ubuntu")
    run("linuxmint", "22", "amd64", "x86_64", error="Use Ubuntu")

    # Exercise the piped headless orchestration with real checksum validation,
    # inert package-manager/setup commands, and explicit root/non-root identities.
    os_release.write_text('ID=debian\nVERSION_ID=13\n')
    executable("dpkg", 'echo arm64\n')
    executable("uname", 'if [ "$1" = -s ]; then echo Linux; else echo aarch64; fi\n')
    executable("id", '''
if [ "$1" = -un ]; then echo portrelay-test
elif [ "$#" = 1 ]; then echo "$PORTRELAY_TEST_UID"
elif [ "$2" = root ]; then echo 0
elif [ "$2" = portrelay-test ]; then echo 1000
else exit 1; fi
''')
    calls = fixture / "calls"
    executable("apt-get", 'printf "apt %s\\n" "$*" >> "$PORTRELAY_TEST_CALLS"\n')
    executable("sudo", 'exec "$@"\n')
    executable("setup-headless", 'printf "setup %s\\n" "$*" >> "$PORTRELAY_TEST_CALLS"\n')
    headless_source = source.replace('/etc/os-release', str(os_release)).replace(
        '/run/systemd/system', str(fixture)).replace(
        '/usr/lib/portrelay/setup-headless', str(fixture / 'setup-headless'))
    payload = 'valid package fixture'
    digest = hashlib.sha256(payload.encode()).hexdigest()
    headless_source = re.sub(r'checksum=(?:PENDING_[A-Z0-9_]+|[a-f0-9]{64})',
                             f'checksum={digest}', headless_source)
    headless_env = {**env, 'SUDO_USER': '', 'PORTRELAY_TEST_PAYLOAD': payload,
                    'PORTRELAY_TEST_CALLS': str(calls)}
    for uid, args, success in (
        ('1000', ['--headless'], True),
        ('0', ['--headless', '--user', 'portrelay-test'], True),
        ('0', ['--headless'], False),
        ('0', ['--headless', '--user', 'root'], False),
        ('0', ['--headless', '--user', '-bad'], False),
    ):
        calls.unlink(missing_ok=True)
        marker.unlink(missing_ok=True)
        result = subprocess.run(['sh', '-s', '--', *args], input=headless_source,
                                env={**headless_env, 'PORTRELAY_TEST_UID': uid},
                                text=True, capture_output=True)
        if success:
            assert result.returncode == 0, result.stderr
            lines = calls.read_text().splitlines()
            assert lines[0] == 'apt update', lines
            assert lines[1].startswith('apt install --no-install-recommends -y '), lines
            assert lines[2:] == ['setup portrelay-test'], lines
            assert not Path(marker.read_text().splitlines()[1]).parent.exists()
        else:
            assert result.returncode != 0, result.stdout
            assert not calls.exists() and not marker.exists(), result.stdout

print("PASS: distro/architecture selection, 32-bit OS on 64-bit kernel, checksum rejection/cleanup, piped headless orchestration, root/owner rejection before download.")
