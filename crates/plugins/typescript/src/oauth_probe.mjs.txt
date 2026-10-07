import assert from 'node:assert/strict';
import { OAuthClientCredentials, OAuthTokenError, createOAuthClient } from './dist/index.js';
let issued = 0;
const provider = new OAuthClientCredentials({tokenUrl:'https://issuer.test/token',clientId:'ü:id',clientSecret:'secret',refreshLeewaySeconds:0,fetch:async(url,init)=>{assert.equal(init.redirect,'error');assert.equal(atob(init.headers.authorization.slice(6)),'%C3%BC%3Aid:secret');assert.equal(new URLSearchParams(init.body).get('grant_type'),'client_credentials');issued++;await new Promise(r=>setTimeout(r,10));return Response.json({access_token:`t${issued}`,token_type:'Bearer',expires_in:3600})}});
assert.deepEqual(await Promise.all(Array.from({length:16},()=>provider.token())),Array(16).fill('t1'));assert.equal(issued,1);
provider.invalidate('obsolete');assert.equal(await provider.token(),'t1');
let calls=0,hooks=0;
const client=createOAuthClient({baseUrl:'https://api.test',middleware:[async(r,next)=>{hooks++;return next(r)}],fetch:async(url,init)=>{calls++;assert.equal(new Headers(init.headers).get('authorization'),calls===1?'Bearer t1':'Bearer t2');return Response.json({}, {status:calls===1?401:200})}},provider);
const req={method:'GET',url:'/things',throwOnError:false,security:[[{id:'oauth',type:'oauth2'}]]};
assert.equal((await client(req)).status,200);assert.equal(calls,2);assert.equal(hooks,2);assert.equal(issued,2);
let denied=0;const deny=createOAuthClient({baseUrl:'https://api.test',fetch:async()=>{denied++;return Response.json({}, {status:401})}},provider);
assert.equal((await deny(req)).status,401);assert.equal(denied,2);
denied=0;await deny({...req,method:'POST'});assert.equal(denied,1);
denied=0;await deny({...req,method:'POST',idempotencyHeader:'X-Request-Key',headers:{'X-Request-Key':' '}});assert.equal(denied,1);
denied=0;await deny({...req,headers:{authorization:'Own token'}});assert.equal(denied,1);
let release;const waiting=new OAuthClientCredentials({tokenUrl:'https://issuer.test',clientId:'id',clientSecret:'secret',fetch:async()=>{await new Promise(r=>release=r);return Response.json({access_token:'wait',token_type:'bearer',expires_in:3600})}});
const controller=new AbortController();const one=waiting.token(controller.signal);const two=waiting.token();controller.abort();await assert.rejects(one,{name:'AbortError'});release();assert.equal(await two,'wait');
let uncached=0;const zero=new OAuthClientCredentials({tokenUrl:'https://issuer.test',clientId:'id',clientSecret:'secret',fetch:async()=>{uncached++;return Response.json({access_token:'zero',token_type:'bearer',expires_in:0})}});await zero.token();await zero.token();assert.equal(uncached,2);
for(const response of [()=>Response.json({access_token:'secret',token_type:'bearer',expires_in:-1}),()=>new Response('secret'.repeat(20000)),()=>Response.json({access_token:'secret',token_type:'MAC',expires_in:1}),()=>new Response('secret',{status:400})]){
const bad=new OAuthClientCredentials({tokenUrl:'https://issuer.test',clientId:'id',clientSecret:'secret',fetch:async()=>response()});await assert.rejects(()=>bad.token(),e=>e instanceof OAuthTokenError&&!e.message.includes('secret'));
}

const throwing=createOAuthClient({baseUrl:'https://api.test',fetch:async()=>Response.json({}, {status:401})},provider);await assert.rejects(()=>throwing({...req,throwOnError:true}),e=>e.status===401);

let scopeCalls=0;const scoped=createOAuthClient({baseUrl:'https://api.test',fetch:async(url,init)=>{scopeCalls++;assert.equal(new Headers(init.headers).get('authorization'),'Own scope');return Response.json({})}},provider);await scoped({...req,requestOptions:{headers:{authorization:'Own scope'}}});assert.equal(scopeCalls,1);
const slowProvider=new OAuthClientCredentials({tokenUrl:'https://issuer.test',clientId:'id',clientSecret:'secret',fetch:async()=>{await new Promise(r=>setTimeout(r,40));return Response.json({access_token:'later',token_type:'bearer',expires_in:3600})}});
const slowClient=createOAuthClient({baseUrl:'https://api.test',fetch:async()=>{throw Error('deadline must stop API transport')}},slowProvider);
await assert.rejects(()=>slowClient({...req,requestOptions:{timeoutMs:5}}),{name:'TimeoutError'});await new Promise(r=>setTimeout(r,50));assert.equal(await slowProvider.token(),'later');
