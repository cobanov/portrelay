#!/usr/bin/env python3
"""Fetch a pinned MIT USB/IP core, then apply PortRelay's reviewed identity patch.
Only the generated target/macos/usbipd-mac-pinned tree is replaced.
"""
import hashlib
from pathlib import Path
import shutil
import subprocess
import tarfile

root = Path(__file__).resolve().parents[1]
revision = '1c2ab4594653db5859d6773bdd010303a641e98e'
digest = 'abc4b35c347578abe248fb853b0fa3d75a47559013aa7f337de7baebaa9ab6e1'
work = root / 'target/macos'
work.mkdir(parents=True, exist_ok=True)
archive = work / 'upstream.tar.gz'
if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
    subprocess.run(['curl', '--fail', '--silent', '--show-error', '--location', '--proto', '=https',
                    '--proto-redir', '=https', '--retry', '2', '--max-time', '120',
                    f'https://codeload.github.com/beriberikix/usbipd-mac/tar.gz/{revision}',
                    '--output', str(archive)], check=True)
assert hashlib.sha256(archive.read_bytes()).hexdigest() == digest, 'Upstream checksum mismatch'
patch = (root / 'macos/identity.patch').read_bytes()
marker = hashlib.sha256(patch + digest.encode() + b'patch-p1-v1').hexdigest()
stage = work / 'usbipd-mac-pinned'
if (stage / '.portrelay-source').is_file() and (stage / '.portrelay-source').read_text() == marker:
    print('Pinned macOS core ready')
else:
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir()
    with tarfile.open(archive) as tar:
        prefix = f'usbipd-mac-{revision}/'
        for member in tar.getmembers():
            assert member.name == prefix[:-1] or member.name.startswith(prefix)
            if not member.name.startswith(prefix):
                continue
            member.name = member.name[len(prefix):]
            if member.name:
                tar.extract(member, stage, filter='data')
    subprocess.run(['/usr/bin/patch', '--dry-run', '-p1', '-i', str(root / 'macos/identity.patch')], cwd=stage, check=True)
    subprocess.run(['/usr/bin/patch', '-p1', '-i', str(root / 'macos/identity.patch')], cwd=stage, check=True)
    (stage / '.portrelay-source').write_text(marker)
    print('Pinned macOS core prepared with device-instance binding')
