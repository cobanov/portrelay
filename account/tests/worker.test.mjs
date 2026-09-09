import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {Database} from '../database.mjs';
import {createService,hash,hex} from '../worker.mjs';
const origin='https://accounts.example.test';
const cookieValue=response=>response.headers.getSetCookie().map(s=>s.split(';')[0]).filter(s=>!s.endsWith('=')).join('; ');

async function fixture() {
  const DB=new Database();DB.sqlite.exec(readFileSync(new URL('../migrations/0001_accounts.sql',import.meta.url),'utf8'));
  let identity=42;const calls=[];
  const service=createService(async (url,options)=>{
    calls.push({url,options});
    if(url==='https://github.com/login/oauth/access_token') return Response.json({access_token:'fixture-github-token',token_type:'bearer'});
    if(url==='https://api.github.com/user') return Response.json({id:identity,login:'user-'+identity});
    throw new Error('Unexpected provider request');
  });
  const env={DB,PUBLIC_URL:origin,GITHUB_CLIENT_ID:'fixture-client',GITHUB_CLIENT_SECRET:'fixture-client-secret'};
  async function request(path,method='GET',data,headers={}) {
    return service.fetch(new Request(origin+path,{method,headers:{...(data?{'content-type':'application/json'}:{}),...headers},...(data?{body:typeof data==='string'?data:JSON.stringify(data)}:{})}),env);
  }
  async function register(name='My computer',existing) {
    const keys=existing?.keys||await crypto.subtle.generateKey('Ed25519',true,['sign','verify']);
    const id=hex(await crypto.subtle.exportKey('raw',keys.publicKey));
    const token=hex(crypto.getRandomValues(new Uint8Array(32)));
    const challenge=await (await request('/api/challenge','POST')).json();
    const payload=JSON.stringify({version:1,purpose:'portrelay-account-enrollment',origin,device_id:id,token_hash:await hash(token),challenge:challenge.id,nonce:challenge.nonce,name,platform:'linux',address:{id,addrs:existing?.addrs||[]}});
    const signature=hex(await crypto.subtle.sign('Ed25519',keys.privateKey,new TextEncoder().encode(payload)));
    const response=await request('/api/register','POST',{payload,signature});
    assert.equal(response.status,200,await response.clone().text());
    const info=await response.json();return {keys,id,token,payload,signature,url:info.authorization_url,headers:{authorization:'Bearer '+token}};
  }
  async function oauth(device,subject=42) {
    identity=subject;
    const login=await request(device.url.slice(origin.length));assert.equal(login.status,302);
    const url=new URL(login.headers.get('location'));assert.equal(url.origin,'https://github.com');
    assert.equal(url.searchParams.get('code_challenge_method'),'S256');assert.equal(url.searchParams.get('scope'),null);
    const path='/callback/github?state='+url.searchParams.get('state')+'&code=fixture-code';
    assert.equal((await request(path)).status,400,'Missing cookie must not consume valid state');
    const response=await request(path,'GET',undefined,{cookie:cookieValue(login)});assert.equal(response.status,303,await response.clone().text());
    assert.equal((await request(path,'GET',undefined,{cookie:cookieValue(login)})).status,400,'OAuth replay');
    const cookies=cookieValue(response);const confirm=await request('/confirm','GET',undefined,{cookie:cookies});
    assert.equal(confirm.status,200);const html=await confirm.text();const csrf=html.match(/name="csrf" value="([a-f0-9]+)"/)[1];
    return {html,csrf,cookies};
  }
  async function confirm(auth,originHeader=origin) {
    return request('/confirm','POST','csrf='+auth.csrf,{cookie:auth.cookies,origin:originHeader,'content-type':'application/x-www-form-urlencoded'});
  }
  async function enroll(name,subject=42,existing) {
    const d=await register(name,existing);const auth=await oauth(d,subject);const r=await confirm(auth);assert.equal(r.status,200,await r.clone().text());return d;
  }
  async function sync(device) {
    return request('/api/sync','POST',{name:'My computer',platform:'linux',address:{id:device.id,addrs:[]}},device.headers);
  }
  return {DB,env,service,request,register,oauth,confirm,enroll,sync,calls};
}

test('real SQL: signed enrollment, OAuth state/PKCE, local token activation and no provider token storage',async()=>{
  const f=await fixture();try{
    const d=await f.register('<Mac & mini>');
    assert.equal((await f.request('/api/register','POST',{payload:d.payload,signature:d.signature})).status,400);
    assert.equal((await f.request('/api/register','POST',{payload:d.payload.replace('<Mac & mini>','Other'),signature:d.signature})).status,403);
    assert.equal((await f.sync(d)).status,401);
    assert.equal((await (await f.request('/api/login','GET',undefined,d.headers)).json()).status,'pending');
    const auth=await f.oauth(d);assert.match(auth.html,/&lt;Mac &amp; mini&gt;/);
    assert.equal((await f.confirm(auth,'https://attacker.example')).status,403);
    assert.equal((await f.confirm({...auth,csrf:'bad'})).status,403);
    const response=await f.confirm(auth);assert.equal(response.status,200);
    assert.equal(response.headers.get('cache-control'),'no-store, no-transform');
    assert.equal((await f.confirm(auth)).status,410);
    const state=await (await f.sync(d)).json();assert.equal(state.account.id,'github:42');assert.equal(state.devices.length,1);
    const row=f.DB.sqlite.prepare('SELECT * FROM devices').get();assert.equal(row.token_hash,await hash(d.token));
    const dump=['devices','accounts','enrollments','oauth_states','confirmations'].map(table=>f.DB.sqlite.prepare('SELECT * FROM '+table).all());
    assert.ok(!JSON.stringify(dump).includes(d.token));assert.ok(!JSON.stringify(dump).includes('fixture-github-token'));
    const exchange=f.calls.find(c=>c.url.endsWith('/access_token'));assert.ok(exchange.options.body.get('code_verifier'));assert.equal(exchange.options.body.get('redirect_uri'),origin+'/callback/github');
  }finally{f.DB.close();}
});
test('account isolation, automatic listing, removal and cross-account device ownership',async()=>{
  const f=await fixture();try{
    const a=await f.enroll('Mac',42),b=await f.enroll('Spark',42),c=await f.enroll('Other account',43);
    assert.equal((await (await f.sync(a)).json()).devices.length,2);
    assert.deepEqual((await (await f.sync(c)).json()).devices.map(d=>d.id),[c.id]);
    assert.equal((await f.request('/api/devices/'+a.id,'DELETE',undefined,c.headers)).status,404);
    const rebind=await f.register('Claim Mac',a),approval=await f.oauth(rebind,43);
    assert.equal((await f.confirm(approval)).status,409);
    assert.equal((await f.sync(a)).status,200,'Failed takeover preserves original token');
    assert.equal((await f.request('/api/devices/'+b.id,'DELETE',undefined,a.headers)).status,200);
    assert.equal((await f.sync(b)).status,401);assert.equal((await (await f.sync(a)).json()).devices.length,1);
    assert.equal((await f.request('/api/devices/'+a.id,'DELETE',undefined,a.headers)).status,200);
    assert.equal((await f.sync(a)).status,401);
  }finally{f.DB.close();}
});
test('expiry, missing provider setup, request bounds, host and token rejection',async()=>{
  const f=await fixture();try{
    assert.equal((await f.service.fetch(new Request('https://attacker.example/health'),f.env)).status,400);
    assert.equal((await f.service.fetch(new Request(origin+'/api/challenge',{method:'POST'}),{...f.env,GITHUB_CLIENT_SECRET:''})).status,503);
    assert.equal((await f.request('/api/register','POST','x'.repeat(17000))).status,413);
    assert.equal((await f.request('/api/sync','POST',{})).status,401);
    const d=await f.register();f.DB.sqlite.exec('UPDATE enrollments SET expires=0');
    assert.equal((await f.request(d.url.slice(origin.length))).status,410);
    assert.equal((await f.request('/api/login','GET',undefined,d.headers)).status,401);
  }finally{f.DB.close();}
});
test('sign-in requests are rate-limited and expired rows are cleaned',async()=>{
  const f=await fixture();try{
    for(let i=0;i<30;i++) assert.equal((await f.request('/api/challenge','POST')).status,200);
    assert.equal((await f.request('/api/challenge','POST')).status,429);
    f.DB.sqlite.exec('UPDATE challenges SET expires=0; UPDATE rate_limits SET expires=0');
    await f.service.scheduled({},f.env);
    assert.equal(f.DB.sqlite.prepare('SELECT count(*) AS n FROM challenges').get().n,0);
    assert.equal(f.DB.sqlite.prepare('SELECT count(*) AS n FROM rate_limits').get().n,0);
  }finally{f.DB.close();}
});
test('cancelled enrollment cannot be confirmed and confirmation body is bounded',async()=>{
 const f=await fixture();try{
  const d=await f.register(),auth=await f.oauth(d);
  const large=await f.service.fetch(new Request(origin+'/confirm',{method:'POST',headers:{Cookie:auth.cookies,Origin:origin},body:'x'.repeat(10000)}),f.env);
  assert.equal(large.status,413);
  assert.equal((await f.request('/api/login','DELETE',undefined,d.headers)).status,200);
  assert.equal((await f.confirm(auth)).status,410);
  assert.equal((await f.request('/api/login','GET',undefined,d.headers)).status,401);
 }finally{f.DB.close();}
});

test('Docker/VPN hosts can register 31 addresses without losing endpoints',async()=>{
 const f=await fixture();try{
  const addrs=Array.from({length:31},(_,i)=>({Ip:`172.18.${i}.1:24816`}));
  const d=await f.register('Many-interface Spark',{addrs});
  const auth=await f.oauth(d);assert.equal((await f.confirm(auth)).status,200);
  const stored=JSON.parse(f.DB.sqlite.prepare('SELECT address FROM devices WHERE id=?').get(d.id).address);
  assert.deepEqual(stored.addrs,addrs);
  const response=await f.request('/api/sync','POST',{name:'Many-interface Spark',platform:'linux',address:stored},d.headers);
  assert.equal(response.status,200);assert.equal((await response.json()).devices[0].address.addrs.length,31);
  stored.addrs=Array.from({length:65},(_,i)=>({Ip:`172.18.${i}.1:24816`}));
  assert.equal((await f.request('/api/sync','POST',{name:'Spark',platform:'linux',address:stored},d.headers)).status,400);
 }finally{f.DB.close();}
});
