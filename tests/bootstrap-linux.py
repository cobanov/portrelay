#!/usr/bin/env python3
"""Fail-closed bootstrap tests on Ubuntu/Debian; never installs a package."""
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
installer = root / "web/install.sh"

with tempfile.TemporaryDirectory(prefix="portrelay-bootstrap-test-") as directory:
    fixture = Path(directory)
    # Leave checksum verification real. Substitute only the HTTP download.
    curl = fixture / "curl"
    curl.write_text('''#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = --output ]; then
    printf 'corrupt package' > "$2"
    printf '%s' "$2" > "$PORTRELAY_TEST_DOWNLOAD"
    exit 0
  fi
  shift
done
exit 1
''')
    curl.chmod(0o755)
    marker = fixture / "download-path"
    env = {**os.environ, "PATH": f"{fixture}:{os.environ['PATH']}",
           "PORTRELAY_TEST_DOWNLOAD": str(marker)}
    result = subprocess.run(["sh", str(installer), "--download-only"],
                            env=env, text=True, capture_output=True)
    assert result.returncode != 0, result.stdout
    assert "checksum mismatch" in result.stderr, result.stderr
    assert marker.exists(), result.stderr
    assert not Path(marker.read_text()).parent.exists(), "Download directory was retained"

    # Fail before downloading if a machine has no published architecture.
    marker.unlink()
    dpkg = fixture / "dpkg"
    dpkg.write_text("#!/bin/sh\necho arm64\n")
    dpkg.chmod(0o755)
    result = subprocess.run(["sh", str(installer), "--download-only"],
                            env=env, text=True, capture_output=True)
    assert result.returncode != 0 and "ARM packages" in result.stderr, result.stderr
    assert not marker.exists(), "Unsupported architecture downloaded a package"

print("PASS: corrupt packages rejected, temporary files removed, unsupported architecture rejected before download.")
