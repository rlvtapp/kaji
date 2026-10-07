import test from 'node:test';
import assert from 'node:assert/strict';
import {fingerprint,syncCollection} from '../sync.mjs';
const source={info:{name:'API',schema:'https://schema.getpostman.com/json/collection/v2.1.0/collection.json'},item:[]};
const remote={...source,item:[{name:'existing'}]};
function mock(document=remote){let current=structuredClone(document);const methods=[];return {methods,fetch:async(url,options)=>{assert.equal(url,'https://api.getpostman.com/collections/owner-uid');assert.equal(options.redirect,'error');methods.push(options.method);if(options.method==='PUT')current=JSON.parse(options.body).collection;return new Response(JSON.stringify({collection:current}));}};}
const input={collection:source,uid:'owner-uid',token:'secret'};
test('default check reports drift without writing',async()=>{const api=mock();const result=await syncCollection(input,api);assert.equal(result.changed,true);assert.equal(result.published,false);assert.deepEqual(api.methods,['GET']);assert.equal(result.remoteHash,fingerprint(remote));});
test('reviewed publication writes and verifies exact collection',async()=>{const api=mock();assert.equal((await syncCollection({...input,mode:'publish',expectedHash:fingerprint(remote)},api)).published,true);assert.deepEqual(api.methods,['GET','PUT','GET']);});
test('manual changes and unreviewed writes are blocked',async()=>{for(const expectedHash of [undefined,'0'.repeat(64)]){const api=mock();await assert.rejects(syncCollection({...input,mode:'publish',expectedHash},api));assert.deepEqual(api.methods,['GET']);}});
test('service metadata and key ordering do not create drift',async()=>{const api=mock({...source,info:{...source.info,_postman_id:'managed'}});assert.equal((await syncCollection(input,api)).changed,false);assert.deepEqual(api.methods,['GET']);});
test('errors do not expose token or response body',async()=>{await assert.rejects(syncCollection(input,{fetch:async()=>new Response('secret',{status:403})}),/^Error: Postman API returned HTTP 403$/);await assert.rejects(syncCollection(input,{fetch:async()=>{throw new Error('secret');}}),/^Error: Postman API request failed$/);});
test('failed readback is a publication failure',async()=>{const api=mock();api.fetch=async()=>new Response(JSON.stringify({collection:remote}));await assert.rejects(syncCollection({...input,mode:'publish',expectedHash:fingerprint(remote)},api),/read-back/);});
test('invalid destination is rejected before network access',async()=>{await assert.rejects(syncCollection({...input,uid:'../other'}, {fetch:()=>assert.fail()}),/UID/);});
