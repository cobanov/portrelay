#!/usr/bin/env python3
"""Write a CycloneDX inventory of locked Cargo dependencies (all targets)."""
import json
import pathlib
import subprocess
import tomllib

metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],text=True))
lock=tomllib.loads(pathlib.Path('Cargo.lock').read_text())
checksums={(p['name'],p['version']):p.get('checksum') for p in lock['package']}
references={p['id']:f"pkg:cargo/{p['name']}@{p['version']}" for p in metadata['packages']}
components=[]
for package in metadata['packages']:
    component={'type':'application' if package['name']=='portrelay' else 'library','name':package['name'],'version':package['version'],'bom-ref':references[package['id']],'purl':references[package['id']]}
    if package.get('license'):component['licenses']=[{'license':{'name':package['license']}}]
    if package.get('repository'):component['externalReferences']=[{'type':'vcs','url':package['repository']}]
    checksum=checksums.get((package['name'],package['version']))
    if checksum:component['hashes']=[{'alg':'SHA-256','content':checksum}]
    components.append(component)
bom={'bomFormat':'CycloneDX','specVersion':'1.5','version':1,'components':components,'dependencies':[{'ref':references[n['id']],'dependsOn':sorted({references[d] for d in n['dependencies']})} for n in metadata['resolve']['nodes']]}
output=pathlib.Path('target/packages/SBOM.cdx.json');output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(bom,indent=2)+'\n')
print(output)
