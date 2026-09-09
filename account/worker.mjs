// Account discovery only. Device traffic never passes through this service.
const encoder = new TextEncoder();
const HEX = /^[0-9a-f]{64}$/;
const ID = /^[0-9a-f]{32}$/;
const COOKIE = '__Host-portrelay-login';
const CONFIRM_COOKIE = '__Host-portrelay-confirm';
const TTL = 600;
const epoch = () => Math.floor(Date.now() / 1000);
export const hash = async text => hex(await crypto.subtle.digest('SHA-256', encoder.encode(text)));
export const hex = bytes => Array.from(new Uint8Array(bytes), b => b.toString(16).padStart(2, '0')).join('');
const random = (size=32) => hex(crypto.getRandomValues(new Uint8Array(size)));
const bytes = text => Uint8Array.from(text.match(/../g), h => parseInt(h,16));
const b64url = value => btoa(String.fromCharCode(...new Uint8Array(value))).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');
const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
class Failure extends Error { constructor(status,message) {super(message);this.status=status;} }
function require(condition, status, message) {if(!condition) throw new Failure(status,message);}
function json(value,status=200) {return new Response(JSON.stringify(value),{status,headers:{'content-type':'application/json; charset=utf-8'}});}
function cookie(name,value,age=TTL) {return `${name}=${value}; Path=/; Max-Age=${age}; HttpOnly; Secure; SameSite=Lax`;}
function readCookie(request,name) {return request.headers.get('cookie')?.split(';').map(p=>p.trim()).find(p=>p.startsWith(name+'='))?.slice(name.length+1)||'';}
function page(title,body,status=200) {
  return new Response(`<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escape(title)} · PortRelay</title><style>body{font:16px/1.6 system-ui,sans-serif;background:#f5f7fb;color:#14223a;margin:0;display:grid;min-height:100dvh;place-items:center}main{margin:24px;padding:36px;max-width:440px;background:white;border:1px solid #dce3ed;border-radius:18px;box-shadow:0 12px 50px #14223a08}h1{font-size:28px;line-height:1.2;letter-spacing:-.7px}p{color:#526984}a{color:#2861eb}button{width:100%;font:inherit;font-weight:650;background:#2861eb;color:white;border:0;border-radius:9px;padding:13px;cursor:pointer}code{overflow-wrap:anywhere;font-size:12px}.brand{color:#2861eb;font-weight:750;letter-spacing:-.5px}.small{font-size:13px}.device{padding:14px 18px;border:1px solid #dce3ed;border-radius:10px;margin:24px 0;overflow-wrap:anywhere}.device p{margin:3px 0}@media(max-width:420px){main{padding:24px;margin:16px}}</style><main><div class="brand">⇄ PortRelay</div><h1>${escape(title)}</h1>${body}</main></html>`,{status,headers:{'content-type':'text/html; charset=utf-8'}});
}
function secured(response) {
  const headers=new Headers(response.headers);
  for(const [key,value] of Object.entries({
    'cache-control':'no-store','referrer-policy':'strict-origin','x-content-type-options':'nosniff','x-frame-options':'DENY',
    'content-security-policy':"default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
    'permissions-policy':'camera=(), microphone=(), geolocation=()',
  })) headers.set(key,value);
  return new Response(response.body,{status:response.status,headers});
}
async function readBody(request,limit=16384) {
  require(Number(request.headers.get('content-length')||0)<=limit,413,'Request too large');
  const reader=request.body?.getReader(); let size=0;const chunks=[];
  if(reader) while(true) {const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>limit){await reader.cancel();throw new Failure(413,'Request too large');}chunks.push(value);}
  const data=new Uint8Array(size);let offset=0;for(const chunk of chunks){data.set(chunk,offset);offset+=chunk.byteLength;}
  try {return new TextDecoder('utf-8',{fatal:true}).decode(data);} catch {throw new Failure(400,'Invalid encoding');}
}
async function body(request) {
  const text=await readBody(request);
  try {return JSON.parse(text);} catch {throw new Failure(400,'Invalid JSON');}
}
function profile(value) {
  require(typeof value.name==='string' && encoder.encode(value.name.trim()).length>0 && encoder.encode(value.name).length<=80 && !/[\u0000-\u001f\u007f]/.test(value.name),400,'Invalid computer name');
  require(['macos','linux','windows'].includes(value.platform),400,'Unsupported platform');
  require(value.address && value.address.id===value.device_id && JSON.stringify(value.address).length<=4096 && Array.isArray(value.address.addrs) && value.address.addrs.length<=16,400,'Invalid computer address');
}
async function token(request) {
  const value=request.headers.get('authorization')?.match(/^Bearer ([0-9a-f]{64})$/)?.[1];
  require(value,401,'Sign in to PortRelay first');return hash(value);
}
async function authenticated(request,env) {
  const digest=await token(request);
  const device=await env.DB.prepare('SELECT devices.*,accounts.provider,accounts.display_name FROM devices JOIN accounts ON accounts.id=devices.account_id WHERE token_hash=?').bind(digest).first();
  require(device,401,'This computer was signed out or removed. Sign in again.');return device;
}
async function rate(request,env) {
  const now=epoch(), window=Math.floor(now/300);
  const id=await hash(`${window}:${request.headers.get('cf-connecting-ip')||'local'}`);
  const row=await env.DB.prepare('INSERT INTO rate_limits(id,count,expires) VALUES(?,1,?) ON CONFLICT(id) DO UPDATE SET count=count+1 RETURNING count').bind(id,now+600).first();
  require(row.count<=30,429,'Too many sign-in attempts. Try again in five minutes.');
}
function configured(env) {return !!env.GITHUB_CLIENT_ID && !!env.GITHUB_CLIENT_SECRET;}
function address(env) {return new URL(env.PUBLIC_URL).origin;}
function callback(env) {return address(env)+'/callback/github';}
function accountInfo(device) {return {id:device.account_id,provider:device.provider,name:device.display_name};}
async function clean(env) {
  const now=epoch();
  await env.DB.batch([
    env.DB.prepare('DELETE FROM oauth_states WHERE expires<?').bind(now),
    env.DB.prepare('DELETE FROM confirmations WHERE expires<?').bind(now),
    env.DB.prepare('DELETE FROM enrollments WHERE expires<?').bind(now),
    env.DB.prepare('DELETE FROM challenges WHERE expires<?').bind(now),
    env.DB.prepare('DELETE FROM rate_limits WHERE expires<?').bind(now),
  ]);
}

export function createService(outbound=fetch) {
  async function providerJson(url,options) {
    const response=await outbound(url,{...options,redirect:'manual',signal:AbortSignal.timeout(10000)});
    require(response.ok,502,'GitHub sign-in did not finish. Start sign-in again.');
    return response.json();
  }
  async function handle(request,env) {
    const url=new URL(request.url), path=url.pathname, method=request.method, now=epoch();
    require(url.origin===address(env),400,'Invalid account service host');
    if(method==='GET' && path==='/health') return json({service:'portrelay-account',version:1,providers:{github:configured(env)}});
    if(method==='GET' && path==='/') return page('Your computers. One account.','<p>Open PortRelay on each computer and choose <strong>Sign in with GitHub</strong>. Your computers appear together automatically.</p><p class="small">Device traffic stays on encrypted peer connections. This service stores your public GitHub identity and computer connection details.</p><a href="https://portrelay.cobanov.dev">Get PortRelay ↗</a>');
    if(method==='POST' && path==='/api/challenge') {
      require(configured(env),503,'GitHub sign-in is being configured. Try again shortly.');
      await rate(request,env);await clean(env);
      const id=random(16),nonce=random();
      await env.DB.prepare('INSERT INTO challenges(id,nonce_hash,expires) VALUES(?,?,?)').bind(id,await hash(nonce),now+120).run();
      return json({id,nonce,expires:now+120});
    }
    if(method==='POST' && path==='/api/register') {
      require(configured(env),503,'GitHub sign-in is unavailable');
      const {payload,signature}=await body(request);
      require(typeof payload==='string' && payload.length<=8192 && typeof signature==='string' && /^[0-9a-f]{128}$/.test(signature),400,'Invalid device proof');
      let value;try{value=JSON.parse(payload);}catch{throw new Failure(400,'Invalid device proof');}
      require(value.version===1 && value.purpose==='portrelay-account-enrollment' && value.origin===address(env) && HEX.test(value.device_id) && HEX.test(value.token_hash) && ID.test(value.challenge) && HEX.test(value.nonce),400,'Invalid device proof');
      profile(value);
      let valid=false;
      try {const key=await crypto.subtle.importKey('raw',bytes(value.device_id),{name:'Ed25519'},false,['verify']);valid=await crypto.subtle.verify('Ed25519',key,bytes(signature),encoder.encode(payload));}catch{}
      require(valid,403,'Device identity proof failed');
      const consumed=await env.DB.prepare('DELETE FROM challenges WHERE id=? AND nonce_hash=? AND expires>=? RETURNING id').bind(value.challenge,await hash(value.nonce),now).first();
      require(consumed,400,'Device challenge expired or was already used');
      const id=random(16);
      await env.DB.prepare('INSERT INTO enrollments(id,device_id,token_hash,name,platform,address,expires) VALUES(?,?,?,?,?,?,?)').bind(id,value.device_id,value.token_hash,value.name.trim(),value.platform,JSON.stringify(value.address),now+TTL).run();
      return json({authorization_url:address(env)+'/login?request='+id,expires:now+TTL});
    }
    if(method==='GET' && path==='/login') {
      require(configured(env),503,'GitHub sign-in is unavailable');
      const id=url.searchParams.get('request');require(ID.test(id||''),400,'Invalid sign-in request');
      const enrollment=await env.DB.prepare('SELECT * FROM enrollments WHERE id=? AND expires>=?').bind(id,now).first();
      require(enrollment,410,'This sign-in link expired. Start again in PortRelay.');
      // One OAuth flow per device request; prior browser attempts cannot race approval.
      await env.DB.prepare('DELETE FROM oauth_states WHERE enrollment_id=?').bind(id).run();
      const state=random(),browserCookie=random(),verifier=random();
      await env.DB.prepare('INSERT INTO oauth_states(state_hash,enrollment_id,cookie_hash,verifier,expires) VALUES(?,?,?,?,?)').bind(await hash(state),id,await hash(browserCookie),verifier,now+TTL).run();
      const redirect=new URL('https://github.com/login/oauth/authorize');
      redirect.search=new URLSearchParams({client_id:env.GITHUB_CLIENT_ID,redirect_uri:callback(env),state,code_challenge:b64url(await crypto.subtle.digest('SHA-256',encoder.encode(verifier))),code_challenge_method:'S256'}).toString();
      // No repository, organization, email or write scopes are requested.
      return new Response(null,{status:302,headers:{location:redirect.href,'set-cookie':cookie(COOKIE,browserCookie)}});
    }
    if(method==='GET' && path==='/callback/github') {
      require(configured(env),503,'GitHub sign-in is unavailable');
      const state=url.searchParams.get('state'),browserCookie=readCookie(request,COOKIE),code=url.searchParams.get('code');
      require(HEX.test(state||'') && HEX.test(browserCookie),400,'Sign-in state is missing. Start again in PortRelay.');
      const flow=await env.DB.prepare('DELETE FROM oauth_states WHERE state_hash=? AND cookie_hash=? AND expires>=? RETURNING *').bind(await hash(state),await hash(browserCookie),now).first();
      require(flow,400,'Sign-in expired or was already used');
      require(code && code.length<=512 && !url.searchParams.has('error'),400,'GitHub sign-in was cancelled. Start again in PortRelay.');
      const credentials=await providerJson('https://github.com/login/oauth/access_token',{method:'POST',headers:{'content-type':'application/x-www-form-urlencoded','accept':'application/json'},body:new URLSearchParams({client_id:env.GITHUB_CLIENT_ID,client_secret:env.GITHUB_CLIENT_SECRET,code,redirect_uri:callback(env),code_verifier:flow.verifier})});
      require(typeof credentials.access_token==='string' && credentials.access_token.length<=512 && credentials.token_type?.toLowerCase()==='bearer',502,'GitHub sign-in failed');
      const user=await providerJson('https://api.github.com/user',{headers:{Authorization:'Bearer '+credentials.access_token,Accept:'application/vnd.github+json','User-Agent':'PortRelay-account','X-GitHub-Api-Version':'2022-11-28'}});
      require(Number.isSafeInteger(user.id) && user.id>0 && typeof user.login==='string' && /^[a-zA-Z0-9-]{1,39}$/.test(user.login),502,'GitHub did not return a valid identity');
      const session=random();
      await env.DB.prepare('INSERT INTO confirmations(session_hash,enrollment_id,subject,display_name,expires) VALUES(?,?,?,?,?)').bind(await hash(session),flow.enrollment_id,String(user.id),user.login,now+TTL).run();
      // GitHub access tokens are never persisted or returned to a desktop app.
      const headers=new Headers({location:address(env)+'/confirm'});
      headers.append('set-cookie',cookie(COOKIE,'',0));headers.append('set-cookie',cookie(CONFIRM_COOKIE,session));
      return new Response(null,{status:303,headers});
    }
    if((method==='GET'||method==='POST') && path==='/confirm') {
      const session=readCookie(request,CONFIRM_COOKIE);require(HEX.test(session),400,'Start sign-in in PortRelay first');
      const pending=await env.DB.prepare('SELECT confirmations.subject,confirmations.display_name,enrollments.* FROM confirmations JOIN enrollments ON enrollments.id=confirmations.enrollment_id WHERE session_hash=? AND confirmations.expires>=? AND enrollments.expires>=?').bind(await hash(session),now,now).first();
      require(pending,410,'This sign-in request expired or was already used');
      const csrf=await hash(session+':confirm');
      if(method==='GET') return page('Add this computer?',`<p>Signed in as <strong>${escape(pending.display_name)}</strong> on GitHub.</p><div class="device"><strong>${escape(pending.name)}</strong><p>${escape(pending.platform)}</p><code>${escape(pending.device_id.slice(0,16))}</code></div><p>Add only the computer where you just chose Sign in.</p><form method="post" action="/confirm"><input type="hidden" name="csrf" value="${csrf}"><button>Add ${escape(pending.name)}</button></form><p class="small">Your other signed-in computers can discover this computer. USB sharing and receiving input keep their own controls.</p>`);
      require(request.headers.get('origin')===address(env),403,'Invalid confirmation origin');
      const text=await readBody(request,256);
      require(new URLSearchParams(text).get('csrf')===csrf,403,'Invalid confirmation');
      const account='github:'+pending.subject;
      await env.DB.prepare('INSERT INTO accounts(id,provider,subject,display_name,created_at) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET display_name=excluded.display_name').bind(account,'github',pending.subject,pending.display_name,now).run();
      const added=await env.DB.prepare(`INSERT INTO devices(id,account_id,token_hash,name,platform,address,created_at,last_seen)
        SELECT ?,?,?,?,?,?,?,? WHERE (SELECT count(*) FROM devices WHERE account_id=?)<64 OR EXISTS(SELECT 1 FROM devices WHERE id=? AND account_id=?)
        ON CONFLICT(id) DO UPDATE SET token_hash=excluded.token_hash,name=excluded.name,platform=excluded.platform,address=excluded.address,last_seen=excluded.last_seen
        WHERE devices.account_id=excluded.account_id RETURNING id`).bind(pending.device_id,account,pending.token_hash,pending.name,pending.platform,pending.address,now,now,account,pending.device_id,account).first();
      require(added,409,'This computer belongs to another account, or your account reached 64 computers. Sign it out first.');
      await env.DB.prepare('DELETE FROM enrollments WHERE id=?').bind(pending.id).run();
      const result=page('Computer added.',`<p><strong>${escape(pending.name)}</strong> is registered to <strong>${escape(pending.display_name)}</strong>.</p><p>Return to PortRelay. Sign in with the same GitHub account on your other computers; they appear automatically.</p>`);
      result.headers.set('set-cookie',cookie(CONFIRM_COOKIE,'',0));return result;
    }
    if(method==='DELETE' && path==='/api/login') {
      const digest=await token(request);
      await env.DB.prepare('DELETE FROM enrollments WHERE token_hash=?').bind(digest).run();
      return json({cancelled:true});
    }
    if(method==='GET' && path==='/api/login') {
      const digest=await token(request);
      const device=await env.DB.prepare('SELECT devices.*,accounts.provider,accounts.display_name FROM devices JOIN accounts ON accounts.id=devices.account_id WHERE token_hash=?').bind(digest).first();
      if(device) return json({status:'connected',account:accountInfo(device)});
      const pending=await env.DB.prepare('SELECT expires FROM enrollments WHERE token_hash=? AND expires>=?').bind(digest,now).first();
      require(pending,401,'Sign-in expired or this computer was removed. Start sign-in again.');
      return json({status:'pending',expires:pending.expires});
    }
    if(method==='POST' && path==='/api/sync') {
      const device=await authenticated(request,env), value=await body(request);
      value.device_id=device.id;profile(value);
      await env.DB.prepare('UPDATE devices SET name=?,platform=?,address=?,last_seen=? WHERE id=? AND token_hash=?').bind(value.name.trim(),value.platform,JSON.stringify(value.address),now,device.id,device.token_hash).run();
      const list=await env.DB.prepare('SELECT id,name,platform,address,last_seen FROM devices WHERE account_id=? ORDER BY name,id LIMIT 64').bind(device.account_id).all();
      return json({account:accountInfo(device),devices:list.results.map(d=>({...d,address:JSON.parse(d.address),online:now-d.last_seen<=60})),checked_at:now});
    }
    if(method==='DELETE' && path.startsWith('/api/devices/')) {
      const device=await authenticated(request,env),id=path.slice('/api/devices/'.length);
      require(HEX.test(id),400,'Invalid computer');
      const removed=await env.DB.prepare('DELETE FROM devices WHERE id=? AND account_id=? RETURNING id').bind(id,device.account_id).first();
      require(removed,404,'Computer not found in this account');return json({removed:true});
    }
    throw new Failure(404,'Not found');
  }
  return {
    async fetch(request,env) {
      try {return secured(await handle(request,env));}
      catch(error) {
        const status=error instanceof Failure?error.status:500;
        if(status===500) console.error("Account failure",new URL(request.url).pathname,error.name);
        const message=error instanceof Failure?error.message:'Account service is unavailable. Try again shortly.';
        const browser=!new URL(request.url).pathname.startsWith('/api/');
        return secured(browser?page('Unable to finish sign-in',`<p>${escape(message)}</p>`,status):json({error:message},status));
      }
    },
    async scheduled(_event,env) {await clean(env);},
  };
}
export default createService();
