#!/usr/bin/env python3
"""Create a user-owned GitHub sign-in app without exposing credentials to output.
Run locally, open the printed loopback URL, and review GitHub's creation page.
Only the OAuth client ID and secret are saved, directly to Cloudflare secrets.
"""
import hashlib, hmac, html, json, secrets, subprocess, threading, time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse
from urllib.request import Request, urlopen

ROOT=Path(__file__).resolve().parents[1]
STATE=secrets.token_hex(32)
START=time.monotonic()
completed=False
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args): pass
    def page(self,status,body):
        data=('<!doctype html><meta charset="utf-8"><title>PortRelay GitHub setup</title><main style="max-width:600px;margin:80px auto;font:17px system-ui">'+body+'</main>').encode()
        self.send_response(status)
        self.send_header('Content-Type','text/html; charset=utf-8')
        self.send_header('Cache-Control','no-store')
        self.send_header('Referrer-Policy','no-referrer')
        self.send_header('Content-Length',str(len(data)))
        self.end_headers();self.wfile.write(data)
    def do_GET(self):
        global completed
        url=urlparse(self.path);query=parse_qs(url.query)
        if url.path=='/':
            manifest={'name':'PortRelay Connect','url':'https://portrelay.cobanov.dev','description':'Sign in to PortRelay and register your computers. No repository permissions.','hook_attributes':{'url':'https://portrelay-account.cobanov.dev/webhook','active':False},'redirect_url':f'http://127.0.0.1:{self.server.server_port}/setup-return','callback_urls':['https://portrelay-account.cobanov.dev/callback/github'],'public':True,'default_permissions':{},'default_events':[],'request_oauth_on_install':False}
            self.page(200,'<h1>PortRelay GitHub sign-in</h1><p>Create a GitHub App owned by your account, with no repository or organization permissions. Its OAuth credentials go directly to the PortRelay account Worker.</p><form method="post" action="https://github.com/settings/apps/new?state='+STATE+'"><input type="hidden" name="manifest" value="'+html.escape(json.dumps(manifest),quote=True)+'"><button>Review on GitHub</button></form>');return
        if url.path!='/setup-return' or completed or time.monotonic()-START>3600 or not hmac.compare_digest(query.get('state',[''])[0],STATE):
            self.page(400,'Invalid or expired setup');return
        code=query.get('code',[''])[0]
        if not code or len(code)>256 or not code.isalnum():self.page(400,'Invalid setup code');return
        completed=True
        try:
            req=Request('https://api.github.com/app-manifests/'+code+'/conversions',method='POST',data=b'',headers={'Accept':'application/vnd.github+json','User-Agent':'PortRelay-setup'})
            with urlopen(req,timeout=20) as response:result=json.load(response)
            for key,field in [('GITHUB_CLIENT_ID','client_id'),('GITHUB_CLIENT_SECRET','client_secret')]:
                value=result[field]
                run=subprocess.run(['npx','--yes','wrangler@4.105.0','secret','put',key,'--config','account/wrangler.toml'],cwd=ROOT,input=value+'\n',text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=120)
                if run.returncode:raise RuntimeError('Cloudflare secret storage failed')
            print('GitHub App created and OAuth credentials saved to Cloudflare. App: '+result.get('html_url',''),flush=True)
            self.page(200,'<h1>GitHub sign-in is configured.</h1><p>PortRelay can now register computers. You can close this tab.</p>')
        except Exception:
            print('Setup did not finish. Inspect GitHub app settings and Cloudflare configuration; no credentials were printed.',flush=True)
            self.page(500,'Setup did not finish. Check the local setup terminal.');return
        threading.Thread(target=self.server.shutdown,daemon=True).start()
server=HTTPServer(('127.0.0.1',0),Handler)
print(f'Open http://127.0.0.1:{server.server_port}/ to review GitHub App setup.',flush=True)
expiry=threading.Timer(3600,server.shutdown)
expiry.daemon=True
expiry.start()
server.serve_forever()
server.server_close()
expiry.cancel()
