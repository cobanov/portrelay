#!/usr/bin/env python3
"""Collect upstream license/notice files from the locked Cargo dependency sources."""
import json
import pathlib
import subprocess

metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],text=True,encoding='utf-8'))
output=pathlib.Path('target/packages/THIRD_PARTY_NOTICES.txt')
output.parent.mkdir(parents=True,exist_ok=True)
parts=['PortRelay Rust dependency notices\n\nThis inventory includes all locked Cargo platforms and test dependencies.\nLinux USB/IP tools and kernel drivers are installed separately by the operating system.\n']
for package in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
    if package['name']=='portrelay':continue
    root=pathlib.Path(package['manifest_path']).parent
    files=set()
    if package.get('license_file'):files.add(root/package['license_file'])
    for p in root.rglob('*'):
        if p.is_file() and p.name.upper().startswith(('LICENSE','COPYING','NOTICE')):
            files.add(p)
    parts.append('\n'+'='*72+'\n'+package['name']+' '+package['version']+'\nDeclared license: '+str(package.get('license'))+'\nSource: '+str(package.get('repository') or package.get('homepage'))+'\n')
    for p in sorted(files):
        parts.append('\n--- '+str(p.relative_to(root))+' ---\n'+p.read_text(encoding='utf-8',errors='replace')+'\n')
output.write_text(''.join(parts),encoding='utf-8')
print(str(output))
