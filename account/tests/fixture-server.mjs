// Local integration fixture only. Never deployed or imported by the Worker/server.
import {createServer} from 'node:http';
import {Readable} from 'node:stream';
import {readFileSync} from 'node:fs';
import {Database} from '../database.mjs';
import {createService} from '../worker.mjs';
const DB=new Database();DB.sqlite.exec(readFileSync(new URL('../migrations/0001_accounts.sql',import.meta.url),'utf8'));
const service=createService(async(url,options)=>new Response(JSON.stringify(url.endsWith('/user')?{id:options.headers.Authorization==='Bearer other'?43:42,login:options.headers.Authorization==='Bearer other'?'other':'fixture'}:{access_token:options.body.get('code')==='other'?'other':'fixture',token_type:'bearer'}),{headers:{'content-type':'application/json'}}));
const server=createServer(async(req,res)=>{try{
 const origin='http://127.0.0.1:'+server.address().port;
 const request=new Request(new URL(req.url,origin),{method:req.method,headers:req.headers,...(req.method==='GET'?{}:{body:Readable.toWeb(req),duplex:'half'})});
 const response=await service.fetch(request,{DB,PUBLIC_URL:origin,GITHUB_CLIENT_ID:'fixture',GITHUB_CLIENT_SECRET:'fixture'});
 res.statusCode=response.status;response.headers.forEach((v,k)=>{if(k!=='set-cookie')res.setHeader(k,v)});res.setHeader('set-cookie',response.headers.getSetCookie());
 res.end(Buffer.from(await response.arrayBuffer()));
}catch{res.writeHead(500);res.end();}});
server.listen(0,'127.0.0.1',()=>console.log('http://127.0.0.1:'+server.address().port));
