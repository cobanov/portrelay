#!/usr/bin/env python3
"""Crash/restart checks for the disposable VM setup documented in validation.md."""
import argparse
import importlib.util
import json
import pathlib
import sys

spec=importlib.util.spec_from_file_location('smoke',pathlib.Path(__file__).with_name('two-host-smoke.py'))
smoke=importlib.util.module_from_spec(spec);spec.loader.exec_module(smoke)
parser=argparse.ArgumentParser()
parser.add_argument('--ssh-config',required=True)
parser.add_argument('--exporter',required=True)
parser.add_argument('--importer',required=True)
parser.add_argument('--device',required=True)
parser.add_argument('--export-state',default='portrelay')
parser.add_argument('--export-helper',default='portrelay-helper.service')
parser.add_argument('--import-agent',default='portrelay-test-agent.service')
parser.add_argument('--confirm-test-hosts',action='store_true',required=True,help='Confirm both are designated test hosts; this kills the named test processes')
args=parser.parse_args()
smoke.SSH_OPTIONS=['-F',args.ssh_config,'-o','ControlMaster=auto','-o','ControlPersist=60','-o','ControlPath=/tmp/portrelay-smoke-%C']
a,b=args.exporter,args.importer
smoke.STATE_DIRS[a]=args.export_state
owner,client=smoke.api(a),smoke.api(b)
for s in [owner,client]:
    assert s['network']=='Relay only · encrypted'
    assert s['address']['addrs'] and all('Relay' in address for address in s['address']['addrs'])
print(json.dumps({'check':'forced_internet_relay','no_direct_ip_transports':True}),flush=True)
ticket=smoke.api(a,{'op':'invite'})['invitation']
smoke.api(b,{'op':'pair','invitation':ticket})
smoke.api(a,{'op':'approve','peer':client['id']})
device=next(d for d in smoke.api(a)['devices'] if d['id']==args.device)
smoke.api(a,{'op':'share','device':device['id'],'peer':client['id']})
connect={'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']}
smoke.api(b,connect);smoke.wait_for(lambda:smoke.remote(b,smoke.FIND_TTY))
smoke.remote(a,"import subprocess,json,sys;subprocess.run(['systemctl','kill','--signal=KILL','--kill-whom=main',json.load(sys.stdin)],check=True);print(json.dumps({'killed':True}))",args.export_helper,sudo=True)
smoke.wait_for(lambda:not smoke.api(a)['sessions'] and not smoke.api(b)['sessions'])
smoke.wait_for(lambda:smoke.api(a)['helper_ready'])
smoke.wait_for(lambda:not smoke.remote(b,smoke.FIND_TTY))
remaining=smoke.remote(a,"import glob,json;print(json.dumps(glob.glob('/run/portrelay/*.json')))",sudo=True)
assert remaining==[],remaining
smoke.assert_cleanup(a,b,device['id'])
print(json.dumps({'check':'helper_crash_restart_recovery','passed':True}),flush=True)
# After successful recovery, the same permission can support a manual new loan.
smoke.api(b,connect);smoke.wait_for(lambda:smoke.remote(b,smoke.FIND_TTY))
smoke.remote(b,"import subprocess,json,sys;subprocess.run(['systemctl','kill','--signal=KILL','--kill-whom=main',json.load(sys.stdin)],check=True);print(json.dumps({'killed':True}))",args.import_agent,sudo=True)
smoke.wait_for(lambda:not smoke.api(a)['sessions'],timeout=35)
smoke.wait_for(lambda:not smoke.remote(b,smoke.FIND_TTY))
for host,directory in [(a,'/run/portrelay'),(b,'/run/portrelay-test')]:
    remaining=smoke.remote(host,"import glob,json,sys;directory=json.load(sys.stdin);print(json.dumps(glob.glob(directory+'/*.json')))",directory,sudo=True)
    assert remaining==[],remaining
driver=smoke.remote(a,"import pathlib,json,sys;print(json.dumps((pathlib.Path('/sys/bus/usb/devices')/json.load(sys.stdin)/'driver').resolve().name))",device['id'])
assert driver=='usb',driver
print(json.dumps({'check':'agent_crash_cleanup','passed':True}),flush=True)
