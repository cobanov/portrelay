#!/usr/bin/env python3
"""Opt-in two-host input acceptance against isolated agents and Linux test helper.

No physical USB devices are accessed. All injected input is grabbed by the
test reader before sending, so it cannot click/type in an existing desktop app.
"""
import argparse
import json
from pathlib import Path
import shlex
import subprocess
import threading
import time
import urllib.request
import urllib.error

parser=argparse.ArgumentParser()
parser.add_argument('--local-state',required=True)
parser.add_argument('--host',required=True)
parser.add_argument('--remote-state',required=True)
parser.add_argument('--capture-script',required=True)
args=parser.parse_args()
RPC = '''import json,pathlib,sys,urllib.request,urllib.error
p=json.load(sys.stdin); a=json.loads((pathlib.Path(p['state'])/'api.json').read_text())
r=urllib.request.Request('http://127.0.0.1:'+str(a['port'])+('/api/state' if p['action'] is None else '/api/action'),headers={'Authorization':'Bearer '+a['token'],'Content-Type':'application/json'},data=None if p['action'] is None else json.dumps(p['action']).encode())
try:
 with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(r,timeout=30) as response: print(response.read().decode())
except urllib.error.HTTPError as error: print(error.read().decode())
'''
def ssh(command, **kwargs):
    return subprocess.run(['ssh','-o','BatchMode=yes','-o','ConnectTimeout=8',args.host,command],capture_output=True,text=True,timeout=35,check=True,**kwargs).stdout
def remote(action=None):
    result=json.loads(ssh('python3 -c '+shlex.quote(RPC),input=json.dumps({'state':args.remote_state,'action':action})))
    if 'error' in result: raise RuntimeError(result['error'])
    return result
def local(action=None):
    a=json.loads((Path(args.local_state)/'api.json').read_text())
    r=urllib.request.Request('http://127.0.0.1:'+str(a['port'])+('/api/state' if action is None else '/api/action'),headers={'Authorization':'Bearer '+a['token'],'Content-Type':'application/json'},data=None if action is None else json.dumps(action).encode())
    try:
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(r,timeout=25) as response: return json.load(response)
    except urllib.error.HTTPError as error: raise RuntimeError(json.load(error).get('error')) from None
def rejected(call):
    try: call()
    except RuntimeError: return
    raise AssertionError('Operation unexpectedly allowed')
def wait(call, seconds=12):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=call()
        if value: return value
        time.sleep(.1)
    raise AssertionError('Timed out waiting for cleanup')
class Control:
    def __init__(self,peer):
        self.id=local({'op':'input_start','peer':peer})['session']
        self.sequence=1; self.lock=threading.Lock(); self.done=threading.Event(); self.error=None
        self.thread=threading.Thread(target=self.heartbeat,daemon=True); self.thread.start()
    def send(self,events):
        with self.lock:
            result=local({'op':'input_events','session':self.id,'batch':{'sequence':self.sequence,'events':events}})
            self.sequence+=1; return result
    def heartbeat(self):
        while not self.done.wait(.2):
            try: self.send([])
            except Exception as error: self.error=error; return
    def stop(self):
        self.done.set(); self.thread.join(timeout=3)
        try: local({'op':'disconnect','session':self.id})
        except RuntimeError: pass

receiver=remote(); sender=local(); rid=receiver['id']; sid=sender['id']
assert receiver['input']['ready'],receiver['input']
if rid not in sender['peers']:
    local({'op':'pair','invitation':remote({'op':'invite'})['invitation']})
rejected(lambda: local({'op':'input_start','peer':rid}))
remote({'op':'approve','peer':sid})
remote({'op':'input_allow','peer':sid,'allowed':False})
rejected(lambda: local({'op':'input_start','peer':rid}))
remote({'op':'input_allow','peer':sid,'allowed':True})
print('PASS: pairing and separate receiver authorization',flush=True)
capture_path='/tmp/portrelay-input-'+str(int(time.time()))+'.json'
capture=subprocess.Popen(['ssh','-o','BatchMode=yes',args.host,'sudo -n python3 '+shlex.quote(args.capture_script)+' '+capture_path],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
control=None
try:
    control=Control(rid)
    wait(lambda: ssh('sudo -n test -f '+capture_path.replace('.json','.ready')+' && echo ready || true').strip())
    rejected(lambda: local({'op':'input_start','peer':rid}))
    control.send([{'type':'move','x':17,'y':-9},{'type':'wheel','x':1,'y':-1}])
    for button in range(5): control.send([{'type':'button','button':button,'down':True},{'type':'button','button':button,'down':False}])
    control.send([{'type':'key','code':30,'value':1},{'type':'key','code':30,'value':2},{'type':'key','code':30,'value':0},{'type':'key','code':42,'value':1},{'type':'release'}])
    # Leave a key and button held; disconnect must destroy the virtual devices.
    control.send([{'type':'key','code':29,'value':1},{'type':'button','button':0,'down':True}])
    time.sleep(.1)
    control.stop(); control=None
    out,err=capture.communicate(timeout=15)
    assert capture.returncode==0,(out,err)
    result=json.loads(ssh('sudo -n cat '+capture_path))
    events=result['events']
    for event in [[2,0,17],[2,1,-9],[2,6,1],[2,8,-1],[1,30,1],[1,30,2],[1,30,0],[1,42,0]]:
        assert event in events,(event,events)
    for code in range(272,277):
        assert [1,code,1] in events and [1,code,0] in events
    assert result['devices_removed']
    wait(lambda:not local()['sessions'] and not remote()['sessions'])
    print('PASS: real Mac -> encrypted QUIC -> ARM Linux uinput: motion, both scroll axes, five buttons, key repeat/release, exclusive lease and disconnect cleanup',flush=True)
    control=Control(rid)
    rejected(lambda:local({'op':'input_events','session':control.id,'batch':{'sequence':0,'events':[]}}))
    control.stop();control=None
    wait(lambda:not remote()['sessions'])
    print('PASS: replay rejected and session closed',flush=True)
    control=Control(rid)
    remote({'op':'input_allow','peer':sid,'allowed':False})
    wait(lambda:not remote()['sessions'])
    control.stop();control=None
    rejected(lambda:local({'op':'input_start','peer':rid}))
    remote({'op':'input_allow','peer':sid,'allowed':True})
    print('PASS: live receiver revocation and reconnect denial',flush=True)
    session=local({'op':'input_start','peer':rid})['session']
    wait(lambda:not local()['sessions'] and not remote()['sessions'],8)
    print('PASS: missing browser heartbeat closes both ends',flush=True)
finally:
    if control: control.stop()
    if capture.poll() is None:
        ssh('sudo -n systemctl stop portrelay-input-test')
        capture.communicate(timeout=15)
