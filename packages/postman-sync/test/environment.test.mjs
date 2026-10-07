import {test} from 'node:test';
import assert from 'node:assert/strict';
import {syncEnvironment,mergeEnvironment,environmentFingerprint} from '../environment.mjs';
const variable=(key,value,type='default')=>({key,value,type,enabled:true});
const remote={name:'old',values:[variable('baseUrl','https://old.example'),variable('token','secret-value','secret'),variable('manual','keep')]};
const source={name:'new',values:[variable('baseUrl','https://new.example'),variable('token','','secret')]};
function server(initial=remote,{badReadback=false}={}) {
  let state=structuredClone(initial);const calls=[];
  return {calls,fetch:async(url,options)=>{assert.match(url,/^https:\/\/api.getpostman.com\/environments\/existing-uid$/);calls.push(options);if(options.method==='PUT'){state=JSON.parse(options.body).environment;return new Response('{}');}return new Response(JSON.stringify({environment:badReadback&&calls.length>2?initial:state}));}};
}
const input={environment:source,uid:'existing-uid',token:'api-secret'};
test('readonly check returns hashes and preserves remote secrets and manual variables',async()=>{
  const stub=server();const result=await syncEnvironment(input,stub);assert.equal(result.changed,true);assert.equal(result.published,false);assert.equal(stub.calls.length,1);assert.ok(!JSON.stringify(result).includes('secret-value'));
  const merged=mergeEnvironment(source,remote);assert.equal(merged.values.find(v=>v.key==='token').value,'secret-value');assert.equal(merged.values.find(v=>v.key==='manual').value,'keep');
});
test('reviewed publish reads back portable environment, retaining credentials',async()=>{
  const stub=server();const result=await syncEnvironment({...input,mode:'publish',expectedHash:environmentFingerprint(remote)},stub);assert.equal(result.published,true);assert.deepEqual(stub.calls.map(x=>x.method),['GET','PUT','GET']);const body=JSON.parse(stub.calls[1].body).environment;assert.equal(body.values.find(v=>v.key==='token').value,'secret-value');assert.equal(body.values.find(v=>v.key==='manual').value,'keep');assert.equal(body.id,undefined);
});
test('changed publish needs reviewed hash and rejects remote drift before PUT',async()=>{
  for(const expectedHash of [undefined,'0'.repeat(64)]) {const stub=server();await assert.rejects(syncEnvironment({...input,mode:'publish',expectedHash},stub),/review/);assert.equal(stub.calls.length,1);}
});
test('source cannot upload secrets or downgrade a remote secret',()=>{
  assert.throws(()=>mergeEnvironment({...source,values:[variable('token','leak','secret')]},remote),/empty/);
  assert.throws(()=>mergeEnvironment({...source,values:[variable('token','leak')]},remote),/remote secret/);
  assert.equal(mergeEnvironment({...source,values:[variable('token','')]},remote).values.find(v=>v.key==='token').type,'secret');
});
test('duplicate invalid variables and destination injection fail without a request',async()=>{
  assert.throws(()=>mergeEnvironment({...source,values:[variable('x',''),variable('x','')]},remote),/duplicate/);
  await assert.rejects(syncEnvironment({...input,uid:'../evil'},{fetch:()=>assert.fail()}),/destination/);
});
test('failed readback is never reported as published and errors redact API data',async()=>{
  await assert.rejects(syncEnvironment({...input,mode:'publish',expectedHash:environmentFingerprint(remote)},server(remote,{badReadback:true})),/read-back/);
  await assert.rejects(syncEnvironment(input,{fetch:async()=>new Response('api-secret',{status:403})}),error=>error.message==='Postman API returned HTTP 403');
});
test('service metadata and variable ordering do not change review hashes',()=>{
 assert.equal(environmentFingerprint(remote),environmentFingerprint({...remote,id:'server-id',values:[...remote.values].reverse()}));
});
test('oversized streaming responses are cancelled before publication',async()=>{
 let cancelled=false;
 const stream=new ReadableStream({pull(controller){controller.enqueue(new Uint8Array(1024*1024));},cancel(){cancelled=true;}});
 await assert.rejects(syncEnvironment(input,{fetch:async()=>new Response(stream)}),/exceeds 12 MiB/);
 assert.equal(cancelled,true);
});
test('response read failures redact transport details',async()=>{
 const stream=new ReadableStream({start(controller){controller.error(new Error('api-secret'));}});
 await assert.rejects(syncEnvironment(input,{fetch:async()=>new Response(stream)}),error=>error.message==='Postman API response failed');
});
