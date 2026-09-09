#!/usr/bin/env python3
"""Real Rust agents + real SQLite/WebCrypto; GitHub responses are an explicit fixture.
Verifies no invitation, identity proof interoperability, isolation, revocation,
restart revalidation, sign-out and fail-closed membership expiry. No USB hardware.
"""
import http.client as http_client
import json, os, re, subprocess, sys, tempfile, time
from pathlib import Path
from urllib.parse import urlsplit, parse_qs, urlencode
binary=Path(sys.argv[1]).resolve()
if os.name=='nt' and not binary.exists():binary=binary.with_suffix('.exe')
root=Path(__file__).resolve().parents[1]
processes=[]
def http(url,method='GET',data=None,headers=None):
 u=urlsplit(url);c=http_client.HTTPConnection(u.hostname,u.port,timeout=10)
 c.request(method,u.path+('?' + u.query if u.query else ''),data,headers or {});r=c.getresponse();body=r.read().decode();out=(r.status,dict(r.getheaders()),body);c.close();return out

def wait(fn,seconds=30):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   value=fn()
   if value:return value
  except (ConnectionError,FileNotFoundError,json.JSONDecodeError):pass
  time.sleep(.2)
 raise AssertionError('Timed out')

def state(directory):
 access=json.loads((directory/'api.json').read_text());status,_,body=http(f'http://127.0.0.1:{access["port"]}/api/state',headers={'Authorization':'Bearer '+access['token']});assert status==200;return json.loads(body)
def action(directory,value,ok=True):
 access=json.loads((directory/'api.json').read_text());status,_,body=http(f'http://127.0.0.1:{access["port"]}/api/action','POST',json.dumps(value),{'Authorization':'Bearer '+access['token'],'Content-Type':'application/json'});assert (status==200)==ok,(status,body);return json.loads(body)
def authorize(url,origin,other=False):
 status,headers,_=http(url);assert status==302
 cookie=headers['set-cookie'].split(';')[0];query=parse_qs(urlsplit(headers['location']).query)
 callback=origin+'/callback/github?'+urlencode({'state':query['state'][0],'code':'other' if other else 'fixture'})
 status,headers,_=http(callback,headers={'Cookie':cookie});assert status==303
 # Python's header dictionary retains the last Set-Cookie, the confirmation cookie.
 cookie=headers['set-cookie'].split(';')[0]
 status,_,body=http(origin+'/confirm',headers={'Cookie':cookie});assert status==200
 csrf=re.search(r'name="csrf" value="([a-f0-9]+)"',body)[1]
 status,_,_=http(origin+'/confirm','POST',urlencode({'csrf':csrf}),{'Cookie':cookie,'Origin':origin,'Content-Type':'application/x-www-form-urlencoded'});assert status==200

def start(directory,name,origin):
 p=subprocess.Popen([str(binary),'--data-dir',str(directory),'run','--name',name,'--bind','127.0.0.1:0','--no-open','--helper',str(directory/'missing')],env={**os.environ,'PORTRELAY_ACCOUNT_SERVER':origin},stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);processes.append(p);wait(lambda:directory.joinpath('api.json').exists());wait(lambda:state(directory));return p

try:
 fixture=subprocess.Popen(['node','account/tests/fixture-server.mjs'],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True);processes.append(fixture);origin=fixture.stdout.readline().strip();assert origin.startswith('http://127.0.0.1:')
 with tempfile.TemporaryDirectory(prefix='portrelay-account-') as tmp:
  dirs=[Path(tmp)/n for n in ['Mac fixture','Pi fixture','Other account']]
  agents=[]
  for i,d in enumerate(dirs):
   agents.append(start(d,d.name,origin));reply=action(d,{'op':'account_login'});authorize(reply['authorization_url'],origin,i==2);wait(lambda:state(d)['account']['status']=='connected')
  a,b,c=dirs; aid=state(a)['id'];bid=state(b)['id']
  wait(lambda:bid in state(a)['peers']);wait(lambda:aid in state(b)['peers'])
  assert state(a)['peers'][bid]['approved'] and state(a)['peers'][bid]['account_id']=='github:42'
  assert state(c)['peers']=={} and not state(a)['grants'] and not state(b)['input']['controllers']
  assert 'token' not in state(a)['account']
  status,_,_=http(origin+'/api/sync','POST','{}',{'Content-Type':'application/json'});assert status==401
  print('PASS: native identity proof, browser OAuth fixture, automatic peers, isolation, no automatic grants',flush=True)
  action(a,{'op':'rename','name':'Mac renamed'});wait(lambda:state(b)['peers'][aid]['name']=='Mac renamed')
  action(a,{'op':'revoke','peer':bid});wait(lambda:state(b)['account']['status']=='signed_out');assert not state(b)['peers'];assert bid not in state(a)['peers']
  print('PASS: rename sync and account removal revoke both agents',flush=True)
  reply=action(b,{'op':'account_login'});authorize(reply['authorization_url'],origin);wait(lambda:state(b)['account']['status']=='connected');wait(lambda:bid in state(a)['peers'])
  # Registry loss must not grant cached membership after restart or retain it forever.
  fixture.terminate();fixture.wait(timeout=10)
  agents[1].terminate();agents[1].wait(timeout=10);start(b,b.name,origin)
  assert not state(b)['peers'][aid]['approved']
  action(b,{'op':'approve','peer':aid},ok=False)
  wait(lambda:not state(a)['peers'][bid]['approved'],seconds=95)
  print('PASS: restart requires fresh membership; registry outage expires trust and cannot be manually approved',flush=True)
  action(a,{'op':'account_logout'});assert not state(a)['peers'];assert state(a)['account']['status']=='signed_out'
  print('PASS: local sign-out removes account trust during service outage',flush=True)
finally:
 for p in processes:
  if p.poll() is None:p.terminate()
 for p in processes:
  try:p.wait(timeout=10)
  except subprocess.TimeoutExpired:p.kill();p.wait()
