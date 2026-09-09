// Self-host behind an HTTPS reverse proxy. No Cloudflare account is required.
import { createServer } from 'node:http';
import { readFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { Readable } from 'node:stream';
import service from './worker.mjs';
import { Database } from './database.mjs';
const path=process.env.DATABASE_PATH||'./data/accounts.sqlite';
mkdirSync(dirname(path),{recursive:true,mode:0o700});
const DB=new Database(path);
DB.sqlite.exec('CREATE TABLE IF NOT EXISTS migrations(name TEXT PRIMARY KEY)');
for(const name of ['0001_accounts.sql']) {
  if(!DB.sqlite.prepare('SELECT 1 FROM migrations WHERE name=?').get(name)) {
    DB.sqlite.exec('BEGIN');
    try {DB.sqlite.exec(readFileSync(new URL('./migrations/'+name,import.meta.url),'utf8'));DB.sqlite.prepare('INSERT INTO migrations VALUES(?)').run(name);DB.sqlite.exec('COMMIT');}
    catch(error){DB.sqlite.exec('ROLLBACK');throw error;}
  }
}
const env={DB,PUBLIC_URL:process.env.PUBLIC_URL,GITHUB_CLIENT_ID:process.env.GITHUB_CLIENT_ID,GITHUB_CLIENT_SECRET:process.env.GITHUB_CLIENT_SECRET};
if(!env.PUBLIC_URL?.startsWith('https://')) throw new Error('Set PUBLIC_URL to the external HTTPS origin');
const server=createServer(async (req,res)=>{
  try {
    // Never accept a caller-supplied Cloudflare IP header in a self-hosted deployment.
    const headers=new Headers(Object.entries(req.headers).flatMap(([k,v])=>Array.isArray(v)?v.map(x=>[k,x]):v?[[k,v]]:[]));
    headers.set('cf-connecting-ip',req.socket.remoteAddress||'local');
    const request=new Request(new URL(req.url,env.PUBLIC_URL),{method:req.method,headers,...(['GET','HEAD'].includes(req.method)?{}:{body:Readable.toWeb(req),duplex:'half'})});
    const response=await service.fetch(request,env);
    res.statusCode=response.status;
    response.headers.forEach((v,k)=>{if(k!=='set-cookie')res.setHeader(k,v);});
    const cookies=response.headers.getSetCookie();if(cookies.length)res.setHeader('set-cookie',cookies);
    if(response.body)Readable.fromWeb(response.body).pipe(res);else res.end();
  } catch {res.writeHead(500);res.end('Account service is unavailable');}
});
server.requestTimeout=15000;server.headersTimeout=10000;
server.listen(Number(process.env.PORT||8787),process.env.BIND||'127.0.0.1',()=>console.log('PortRelay account service listening'));
const cleanup=setInterval(()=>service.scheduled({},env,{waitUntil:p=>p.catch(()=>{})}),3600000).unref();
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>{clearInterval(cleanup);server.close(()=>{DB.close();process.exit(0);});});
