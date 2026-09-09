#!/usr/bin/env python3
"""Reject mislabeled Linux packages and mismatched executable versions."""
import os
from pathlib import Path
import shlex
import struct
import subprocess
import sys
import tomllib

binary = Path(sys.argv[1]).resolve()
arch = sys.argv[2]
expected = {
    "amd64": (2, 62), "x86_64": (2, 62),
    "arm64": (2, 183), "aarch64": (2, 183),
    "armhf": (1, 40), "armv7": (1, 40),
}
header = binary.read_bytes()[:64]
if (arch not in expected or len(header) < 52 or header[:4] != b"\x7fELF"
        or header[5] != 1
        or (header[4], struct.unpack_from("<H", header, 18)[0]) != expected[arch]):
    sys.exit(f"Binary is not a little-endian Linux {arch} executable.")
if arch in ("armhf", "armv7") and not struct.unpack_from("<I", header, 36)[0] & 0x400:
    sys.exit("ARM packages require the hard-float ABI.")
version = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
# Build-only escape hatch for a cross-compiled executable, e.g. qemu-arm -L ... .
runner = shlex.split(os.environ.get("PORTRELAY_PACKAGE_RUNNER", ""))
actual = subprocess.check_output([*runner, str(binary), "--version"], text=True, timeout=30).strip()
if actual != f"portrelay {version}":
    sys.exit("Binary and package versions do not match.")
print(version)
