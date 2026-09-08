#!/usr/bin/env python3
"""Exercise real QUIC + Linux kernel USB using an explicitly created serial gadget.

Requires test agents/helpers already running on two SSH-accessible Linux hosts,
with ~/.local/share/portrelay-test state. No physical devices are touched.
"""
import argparse
import base64
import hashlib
import json
import shlex
import subprocess
import time

SSH_OPTIONS=[]
STATE_DIRS={}

REMOTE_API = r'''
import json,sys,urllib.request,pathlib
packet=json.load(sys.stdin)
access=json.loads((pathlib.Path.home()/'.local/share'/packet['directory']/'api.json').read_text())
action=packet['action']
url='http://127.0.0.1:'+str(access['port'])+('/api/state' if action is None else '/api/action')
headers={'Authorization':'Bearer '+access['token'],'Content-Type':'application/json'}
request=urllib.request.Request(url,headers=headers,data=None if action is None else json.dumps(action).encode())
opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
try:
 with opener.open(request,timeout=45) as response: print(response.read().decode())
except urllib.error.HTTPError as error:
 print(error.read().decode());sys.exit(2)
'''

def remote(host, code, data=None, sudo=False, timeout=55):
    command = ('sudo -n ' if sudo else '') + 'python3 -c ' + shlex.quote(code)
    result = subprocess.run(['ssh', *SSH_OPTIONS, '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=8', host, command], input=json.dumps(data), text=True, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f'{host}: {result.stdout.strip()} {result.stderr.strip()}')
    return json.loads(result.stdout)

def api(host, action=None):
    return remote(host, REMOTE_API, {'action':action,'directory':STATE_DIRS.get(host,'portrelay-test')})

def wait_for(fn, timeout=30):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        value=fn()
        if value:return value
        time.sleep(.3)
    raise RuntimeError('Timed out waiting for device state')

def assert_cleanup(owner, client, device, owner_runtime='/run/portrelay', client_runtime='/run/portrelay-test'):
    wait_for(lambda:not api(owner)['sessions'] and not api(client)['sessions'])
    wait_for(lambda:not remote(client,FIND_TTY))
    for host, directory in [(owner,owner_runtime),(client,client_runtime)]:
        state=api(host)
        assert state['helper_ready'],state.get('helper_error',state)
        remaining=remote(host,"import glob,json,sys;print(json.dumps(glob.glob(json.load(sys.stdin)+'/*.json')))",directory,sudo=True)
        assert remaining==[],remaining
    driver=remote(owner,"import pathlib,json,sys;p=pathlib.Path('/sys/bus/usb/devices')/json.load(sys.stdin)/'driver';print(json.dumps(p.resolve().name if p.exists() else None))",device)
    assert driver=='usb',f'Original driver not restored: {driver}'

FIND_TTY = r'''
from pathlib import Path
import json
found=[]
for tty in Path('/sys/class/tty').glob('ttyACM*'):
 p=tty.resolve()
 if 'vhci_hcd' not in str(p):continue
 for parent in p.parents:
  if (parent/'idVendor').exists() and (parent/'idVendor').read_text().strip()=='1d6b' and (parent/'idProduct').read_text().strip()=='0104':
   found.append('/dev/'+tty.name);break
print(json.dumps(found))
'''
ECHO = r'''
import os,tty,select,time
fd=os.open('/dev/ttyGS0',os.O_RDWR|os.O_NOCTTY|os.O_NONBLOCK)
tty.setraw(fd)
end=time.monotonic()+50
while time.monotonic()<end:
 if not select.select([fd],[],[],1)[0]:continue
 try:data=os.read(fd,4096)
 except BlockingIOError:continue
 while data:
  if not select.select([],[fd],[],1)[1]:continue
  try:n=os.write(fd,data);data=data[n:]
  except BlockingIOError:pass
os.close(fd)
'''
TRANSFER = r'''
import os,tty,select,time,json,sys,hashlib
path=json.load(sys.stdin)
fd=os.open(path,os.O_RDWR|os.O_NOCTTY|os.O_NONBLOCK);tty.setraw(fd)
payload=bytes(range(256))*256
received=bytearray();end=time.monotonic()+30
for offset in range(0,len(payload),512):
 block=payload[offset:offset+512];remaining=block
 while remaining:
  if time.monotonic()>end:raise RuntimeError('Serial write timed out')
  if select.select([],[fd],[],.5)[1]:
   try:n=os.write(fd,remaining);remaining=remaining[n:]
   except BlockingIOError:pass
 chunk=bytearray()
 while len(chunk)<len(block):
  if time.monotonic()>end:raise RuntimeError('Serial read timed out')
  if select.select([fd],[],[],.5)[0]:
   try:chunk.extend(os.read(fd,len(block)-len(chunk)))
   except BlockingIOError:pass
 received.extend(chunk)
os.close(fd)
assert bytes(received)==payload,'Payload mismatch'
print(json.dumps({'bytes':len(received),'sha256':hashlib.sha256(received).hexdigest(),'matched':True}))
'''

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--exporter',required=True);parser.add_argument('--importer',required=True);parser.add_argument('--ssh-config');parser.add_argument('--device');parser.add_argument('--export-state',default='portrelay-test');parser.add_argument('--cycles',type=int,default=0);parser.add_argument('--export-address',help='Explicit UDP forwarding address for an isolated NAT test VM');args=parser.parse_args()
    global SSH_OPTIONS
    SSH_OPTIONS=['-o','ControlMaster=auto','-o','ControlPersist=60','-o','ControlPath=/tmp/portrelay-smoke-%C']
    if args.ssh_config:SSH_OPTIONS += ['-F',args.ssh_config]
    a,b=args.exporter,args.importer
    STATE_DIRS[a]=args.export_state
    owner,client=api(a),api(b)
    ticket=api(a,{'op':'invite'})['invitation']
    if args.export_address:
        encoded=ticket.split(':',1)[1]
        invitation=json.loads(base64.urlsafe_b64decode(encoded+'='*(-len(encoded)%4)))
        invitation['address']['addrs']=[{'Ip':args.export_address}]
        ticket='portrelay1:'+base64.urlsafe_b64encode(json.dumps(invitation).encode()).decode().rstrip('=')
    api(b,{'op':'pair','invitation':ticket})
    try:api(b,{'op':'remote','peer':owner['id']})
    except RuntimeError:pass
    else:raise RuntimeError('Unapproved computer could list devices')
    api(a,{'op':'approve','peer':client['id']})
    device=next(d for d in api(a)['devices'] if (d['id']==args.device if args.device else d['vendor']=='1d6b' and d['product']=='0104'))
    api(a,{'op':'share','device':device['id'],'peer':client['id']})
    remote_devices=api(b,{'op':'remote','peer':owner['id']})['devices']
    assert any(d['id']==device['id'] for d in remote_devices)
    connection=api(b,{'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']})
    tty_path=wait_for(lambda:remote(b,FIND_TTY))[0]
    echo=subprocess.Popen(['ssh',*SSH_OPTIONS,'-o','BatchMode=yes',a,'sudo -n python3 -c '+shlex.quote(ECHO)],stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
    try:
        time.sleep(.5)
        result=remote(b,TRANSFER,tty_path,sudo=True)
        print(json.dumps({'check':'kernel_usb_roundtrip','exporter':a,'importer':b,'tty':tty_path,**result}),flush=True)
    finally:
        echo.terminate()
        try:echo.wait(timeout=3)
        except subprocess.TimeoutExpired:echo.kill()
        api(b,{'op':'disconnect','session':connection['session']})
    assert_cleanup(a,b,device['id'])
    print(json.dumps({'check':'disconnect_cleanup','passed':True}),flush=True)
    for cycle in range(args.cycles):
        connection=api(b,{'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']})
        wait_for(lambda:remote(b,FIND_TTY))
        api(b,{'op':'disconnect','session':connection['session']})
        assert_cleanup(a,b,device['id'])
    if args.cycles:print(json.dumps({'check':'repeated_connection_cleanup','cycles':args.cycles,'passed':True}),flush=True)
    # Reconnect and revoke trust from the device owner during an active loan.
    time.sleep(1)
    api(b,{'op':'connect','peer':owner['id'],'device':device['id'],'generation':device['generation']})
    wait_for(lambda:remote(b,FIND_TTY))
    api(a,{'op':'revoke','peer':client['id']})
    assert_cleanup(a,b,device['id'])
    print(json.dumps({'check':'owner_revocation_cleanup','passed':True}),flush=True)

if __name__=='__main__':main()
