#!/usr/bin/env python3
"""Read-only native bundle acceptance. Never opens or claims a USB interface."""
import json
from pathlib import Path
import plistlib
import struct
import subprocess
import sys
import tempfile
import time

app = Path(sys.argv[1]).resolve()
binary = app / 'Contents/MacOS/portrelay'
worker = binary.with_name('portrelay-macos-usb')
info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
assert info['CFBundleExecutable'] == 'portrelay-desktop'
subprocess.run(['codesign', '--verify', '--deep', '--strict', str(app)], check=True)


def call_worker(*args, ok=True):
    result = subprocess.run([str(worker), *args], capture_output=True, timeout=10)
    assert (result.returncode == 0) == ok, result.stderr.decode()
    size = struct.unpack('!I', result.stdout[:4])[0]
    assert 0 < size <= 65536 and len(result.stdout) == size + 4
    return json.loads(result.stdout[4:])


assert call_worker('probe') == {'protocolVersion': 1, 'usbExport': True, 'usbImport': False}
devices = call_worker('inventory')
assert len({d['id'] for d in devices}) == len(devices), 'Ambiguous device IDs'
assert len({d['generation'] for d in devices}) == len(devices), 'Ambiguous generations'
for device in devices:
    assert len(device['generation']) == 64
    assert device['devid'] >> 16 > 0
# A deliberately absent identity cannot touch hardware.
assert 'unplugged' in call_worker('export', '65535-65535', 'a' * 64, ok=False)['error']

with tempfile.TemporaryDirectory(prefix='portrelay-mac-acceptance-') as name:
    directory = Path(name)
    with (directory / 'agent.log').open('w') as log:
        process = subprocess.Popen([str(binary), '--data-dir', name, 'run', '--no-open',
                                    '--name', 'Mac acceptance', '--bind', '127.0.0.1:0'], stdout=log, stderr=log)
        try:
            state = None
            for _ in range(100):
                result = subprocess.run([str(binary), '--data-dir', name, 'status'], capture_output=True, text=True, timeout=12)
                if result.returncode == 0:
                    state = json.loads(result.stdout)
                    break
                time.sleep(.1)
            assert state is not None, (directory / 'agent.log').read_text()
            assert state['platform'] == 'macos' and state['helper_ready'], state.get('helper_error')
            assert state['capabilities']['usb_export'] is True
            assert state['capabilities']['usb_import'] is False
            assert 'entitlement' in state['capabilities']['import_reason']
            result = subprocess.run([str(binary), '--data-dir', name, 'api'], input=json.dumps({
                'op': 'connect', 'peer': 'not-a-peer', 'device': '1-1', 'generation': 'old'}),
                capture_output=True, text=True, timeout=10)
            assert result.returncode != 0 and 'host-controller entitlement' in result.stderr + result.stdout
            subprocess.run([str(binary), '--data-dir', name, 'check'], check=True, capture_output=True, timeout=10)
        finally:
            process.terminate()
            try: process.wait(timeout=12)
            except subprocess.TimeoutExpired: process.kill(); process.wait(); raise
    assert process.returncode == 0
    assert not (directory / 'api.json').exists()
print(f'PASS: signed bundle integrity, native read-only inventory ({len(devices)} devices), absent-device rejection, worker discovery, agent capabilities, early import rejection, and shutdown. No physical USB transfer tested.')
