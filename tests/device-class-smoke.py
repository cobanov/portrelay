#!/usr/bin/env python3
"""Real kernel storage, keyboard/mouse and Ethernet tests in two disposable Linux VMs.

Creates only tests/device-class-gadget.py fixtures. Requires running agents/helpers
and the gadget script at /usr/local/lib/portrelay-qa/device-class-gadget.py on the exporter.
Never use physical devices or a workstation. Both state dirs are portrelay-test.
"""
import argparse, base64, importlib.util, json, pathlib, shlex, subprocess, time
spec = importlib.util.spec_from_file_location('smoke', pathlib.Path(__file__).with_name('two-host-smoke.py'))
s = importlib.util.module_from_spec(spec); spec.loader.exec_module(s)

FIND = r'''
from pathlib import Path
import json,sys
kind=json.load(sys.stdin)
base={'storage':'/sys/class/block','input':'/sys/class/input','network':'/sys/class/net'}[kind]
found=[]
for p in Path(base).iterdir():
 if kind=='input' and not p.name.startswith('event'):continue
 if kind=='storage' and (p/'partition').exists():continue
 path=p.resolve()
 if 'vhci_hcd' not in str(path):continue
 if any((q/'serial').exists() and (q/'serial').read_text().strip()=='PORTRELAY-CLASS-TEST-ONLY' for q in path.parents):found.append(p.name)
print(json.dumps(sorted(found)))
'''
SOURCE_BLOCK = r'''
from pathlib import Path
import json
found=[]
for p in Path('/sys/class/block').iterdir():
 if (p/'partition').exists():continue
 path=p.resolve()
 if 'vhci_hcd' in str(path):continue
 if any((q/'serial').exists() and (q/'serial').read_text().strip()=='PORTRELAY-CLASS-TEST-ONLY' for q in path.parents):found.append('/dev/'+p.name)
assert len(found)<=1,found
print(json.dumps(found[0] if found else None))
'''
STORAGE = r'''
import pathlib,subprocess,json,sys,hashlib
packet=json.load(sys.stdin); dev=packet['device']; mount=pathlib.Path('/mnt/portrelay-qa')
mount.mkdir(exist_ok=True)
subprocess.run(['mount',dev,str(mount)],check=True)
try:
 file=mount/'roundtrip.bin'; payload=bytes(range(256))*1024
 if packet['write']:file.write_bytes(payload);subprocess.run(['sync','-f',str(file)],check=True)
 actual=file.read_bytes();assert actual==payload
 print(json.dumps({'bytes':len(actual),'sha256':hashlib.sha256(actual).hexdigest()}))
finally:subprocess.run(['umount',str(mount)],check=True)
'''

def fixture(owner, kind):
    return s.remote(owner, "import os,subprocess,json,sys,pathlib;subprocess.run(['python3','/usr/local/lib/portrelay-qa/device-class-gadget.py',json.load(sys.stdin)],env=dict(os.environ,PORTRELAY_DISPOSABLE_VM='1'),check=True,stdout=subprocess.DEVNULL);print('true')", kind, sudo=True)

def reject(fn, reason):
    try: fn()
    except RuntimeError as error:
        assert reason.lower() in str(error).lower(),str(error)
    else: raise AssertionError('Unsafe operation was accepted: '+reason)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exporter',required=True);parser.add_argument('--importer',required=True);parser.add_argument('--ssh-config',required=True);parser.add_argument('--export-address',required=True)
    parser.add_argument('--disposable-vms',action='store_true',required=True)
    args=parser.parse_args();a,b=args.exporter,args.importer
    s.SSH_OPTIONS=['-F',args.ssh_config,'-o','ControlMaster=auto','-o','ControlPersist=60','-o','ControlPath=/tmp/portrelay-class-%C']
    owner,client=s.api(a),s.api(b)
    invitation=s.api(a,{'op':'invite'})['invitation'];raw=invitation.split(':',1)[1]
    ticket=json.loads(base64.urlsafe_b64decode(raw+'='*(-len(raw)%4)));ticket['address']['addrs']=[{'Ip':args.export_address}]
    s.api(b,{'op':'pair','invitation':'portrelay1:'+base64.urlsafe_b64encode(json.dumps(ticket).encode()).decode().rstrip('=')})
    s.api(a,{'op':'approve','peer':client['id']})
    for kind,product in [('storage','0105'),('input','0106'),('network','0107')]:
        fixture(a,kind)
        try:
            device=s.wait_for(lambda:next((d for d in s.api(a)['devices'] if d['vendor']=='1d6b' and d['product']==product and kind in d['risks']),None))
            share={'op':'share','device':device['id'],'peer':client['id'],'acknowledge_risks':device['risks']}
            assert kind in device['risks'],device
            if kind=='storage':
                disk=s.wait_for(lambda:s.remote(a,SOURCE_BLOCK,sudo=True))
                s.remote(a,"import subprocess,pathlib,json,sys;pathlib.Path('/mnt/portrelay-qa').mkdir(exist_ok=True);subprocess.run(['mount',json.load(sys.stdin),'/mnt/portrelay-qa'],check=True);print('true')",disk,sudo=True)
                reject(lambda:s.api(a,share),'Unmount')
                s.remote(a,"import subprocess;subprocess.run(['umount','/mnt/portrelay-qa'],check=True);print('true')",sudo=True)
            if kind=='network':
                source_net=s.remote(a,"import pathlib,json,sys;p=pathlib.Path('/sys/bus/usb/devices')/json.load(sys.stdin);print(json.dumps(next(n.name for i in p.iterdir() if (i/'net').exists() for n in (i/'net').iterdir())))",device['id'])
                def admin(state):s.remote(a,"import subprocess,json,sys;p=json.load(sys.stdin);subprocess.run(['ip','link','set',p[0],p[1]],check=True);print('true')",[source_net,state],sudo=True)
                admin('up');reject(lambda:s.api(a,share),'Disable');admin('down')
            reject(lambda:s.api(a,{**share,'acknowledge_risks':[]}), 'Confirm')
            s.api(a,share)
            # A grant is not permission to bypass a later usage change.
            if kind=='storage':
                s.remote(a,"import subprocess,json,sys;subprocess.run(['mount',json.load(sys.stdin),'/mnt/portrelay-qa'],check=True);print('true')",disk,sudo=True)
                reject(lambda:s.api(b,{'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']}),'Unmount')
                s.remote(a,"import subprocess;subprocess.run(['umount','/mnt/portrelay-qa'],check=True);print('true')",sudo=True)
            connection=s.api(b,{'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']})
            incoming=s.wait_for(lambda:(names if len(names := s.remote(b,FIND,kind)) >= (2 if kind=='input' else 1) else None))
            result={}
            if kind=='storage': result=s.remote(b,STORAGE,{'device':'/dev/'+incoming[0],'write':True},sudo=True)
            elif kind=='network':
                gadget_net=s.remote(a,"import pathlib,json;print(json.dumps(pathlib.Path('/sys/kernel/config/usb_gadget/portrelay_class_test/functions/ecm.usb0/ifname').read_text().strip()))")
                for host,interface,addr in [(a,gadget_net,'192.0.2.1/30'),(b,incoming[0],'192.0.2.2/30')]:
                    s.remote(host,"import subprocess,json,sys;i,addr=json.load(sys.stdin);subprocess.run(['ip','addr','replace',addr,'dev',i],check=True);subprocess.run(['ip','link','set',i,'up'],check=True);print('true')",[interface,addr],sudo=True)
                result=s.remote(b,"import subprocess,json;p=subprocess.run(['ping','-c','3','-W','3','192.0.2.1'],capture_output=True,text=True);assert p.returncode==0,p.stdout+p.stderr;print(json.dumps({'ping':p.stdout.strip()}))",sudo=True)
            else:
                read_events=r'''
import os,select,time,struct,json,sys
names=json.load(sys.stdin);fds=[os.open('/dev/input/'+n,os.O_RDONLY|os.O_NONBLOCK) for n in names];events=[];end=time.monotonic()+6
while time.monotonic()<end:
 for fd in select.select(fds,[],[],.3)[0]:
  data=os.read(fd,24*32)
  for offset in range(0,len(data),24):
   _,_,kind,code,value=struct.unpack('llHHi',data[offset:offset+24]);events.append([kind,code,value])
 if [1,30,1] in events and [1,30,0] in events and [2,0,5] in events:break
for fd in fds:os.close(fd)
assert [1,30,1] in events and [1,30,0] in events and [2,0,5] in events,events
print(json.dumps({'keyboard_press_release':True,'mouse_motion':True,'events':events}))
'''
                send=r'''
import os,pathlib,time
root=pathlib.Path('/sys/kernel/config/usb_gadget/portrelay_class_test/functions')
time.sleep(1)
for function,report in [('hid.keyboard',bytes([0,0,4,0,0,0,0,0])),('hid.mouse',bytes([0,5,0]))]:
 major,minor=map(int,(root/function/'dev').read_text().split(':'))
 dev=next(p for p in pathlib.Path('/dev').glob('hidg*') if p.stat().st_rdev==os.makedev(major,minor))
 with dev.open('wb',buffering=0) as f:f.write(report);time.sleep(.2);f.write(bytes(len(report)))
'''
                emitter=subprocess.Popen(['ssh',*s.SSH_OPTIONS,a,'sudo -n python3 -c '+shlex.quote(send)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
                try:result=s.remote(b,read_events,incoming,sudo=True);assert emitter.wait(timeout=8)==0
                finally:
                    if emitter.poll() is None:emitter.kill()
            s.api(b,{'op':'disconnect','session':connection['session']})
            s.wait_for(lambda:not s.api(a)['sessions'] and not s.api(b)['sessions'])
            s.wait_for(lambda:not s.remote(b,FIND,kind))
            if kind=='storage':
                disk=s.wait_for(lambda:s.remote(a,SOURCE_BLOCK,sudo=True));restored=s.remote(a,STORAGE,{'device':disk,'write':False},sudo=True)
                assert restored==result
            for host in (a,b):
                assert s.api(host)['helper_ready']
                assert s.remote(host,"import glob,json;print(json.dumps(glob.glob('/run/portrelay/*.json')))",sudo=True)==[]
            print(json.dumps({'device_class':kind,'consent_and_usage_guards':True,'cleanup':True,**result}),flush=True)
        finally:fixture(a,'stop')

if __name__=='__main__':main()
