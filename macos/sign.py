#!/usr/bin/env python3
"""Sign the three app executables, then seal the bundle. Never unlock or export keys."""
import os
from pathlib import Path
import shlex
import signal
import subprocess

app = Path('target/macos/package/PortRelay.app')
identity = os.environ.get('PORTRELAY_SIGN_IDENTITY', '-')
keychain = os.environ.get('PORTRELAY_SIGN_KEYCHAIN')
original = None
# Keychain APIs can resolve a duplicate private key from another search-list
# entry even when codesign is given --keychain. Limit that lookup for signing,
# then restore the exact user's list, including on failure or interruption.
def interrupted(signum, frame):
    raise KeyboardInterrupt
signal.signal(signal.SIGTERM, interrupted)
try:
    if keychain:
        keychain = str(Path(keychain).expanduser().resolve(strict=True))
        original = shlex.split(subprocess.check_output(['security', 'list-keychains', '-d', 'user'], text=True))
        subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', keychain], check=True)
    options = ['codesign', '--force', '--options', 'runtime', '--sign', identity]
    if identity != '-': options += ['--timestamp']
    if keychain: options += ['--keychain', keychain]
    for name in ('portrelay', 'portrelay-macos-usb', 'portrelay-desktop'):
        subprocess.run(options + [str(app / 'Contents/MacOS' / name)], check=True, timeout=60)
    subprocess.run(options + [str(app)], check=True, timeout=60)
finally:
    if original is not None:
        subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', *original], check=True)
        restored = shlex.split(subprocess.check_output(['security', 'list-keychains', '-d', 'user'], text=True))
        if restored != original: raise RuntimeError('Keychain search list was not restored')
