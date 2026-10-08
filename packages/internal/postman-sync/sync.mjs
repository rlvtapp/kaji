import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';

const stable = value => Array.isArray(value) ? value.map(stable) : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value).sort().map(key => [key, stable(value[key])])) : value;
// Postman adds service-owned identity metadata; compare the portable document.
export function fingerprint(collection) {
  const copy=structuredClone(collection);
  if(copy.info) {delete copy.info._postman_id;delete copy.info._exporter_id;}
  return createHash('sha256').update(JSON.stringify(stable(copy))).digest('hex');
}
function validate(collection) {
  if(!collection || typeof collection.info?.name!=='string' || !collection.info.name.trim() || collection.info.schema!=='https://schema.getpostman.com/json/collection/v2.1.0/collection.json' || !Array.isArray(collection.item)) throw new Error('Expected a Postman Collection 2.1 document');
  if(Buffer.byteLength(JSON.stringify(collection))>10*1024*1024) throw new Error('Collection exceeds 10 MiB');
}
export async function syncCollection(input,{fetch:request=globalThis.fetch}={}) {
  validate(input.collection);
  if(!/^[a-zA-Z0-9_-]+$/.test(input.uid??'')) throw new Error('An explicit destination collection UID is required');
  if(!input.token || /[\r\n]/.test(input.token)) throw new Error('Postman API token is required');
  if(!['check','publish'].includes(input.mode??'check')) throw new Error('Mode must be check or publish');
  const url=`https://api.getpostman.com/collections/${input.uid}`;
  async function call(method,body) {
    let response;
    try {response=await request(url,{method,headers:{'X-Api-Key':input.token,'Content-Type':'application/json'},body:body&&JSON.stringify(body),redirect:'error',signal:AbortSignal.timeout(30000)});} catch {throw new Error('Postman API request failed');}
    if(!response.ok) throw new Error(`Postman API returned HTTP ${response.status}`);
    const text=await response.text();
    if(Buffer.byteLength(text)>12*1024*1024) throw new Error('Postman response exceeds 12 MiB');
    try {return JSON.parse(text);} catch {throw new Error('Postman API returned invalid JSON');}
  }
  const remote=(await call('GET')).collection;
  validate(remote);
  const before=fingerprint(remote), after=fingerprint(input.collection);
  if(input.expectedHash && (!/^[a-f0-9]{64}$/.test(input.expectedHash) || input.expectedHash!==before)) throw new Error('Remote collection changed; review it before publishing');
  if(before===after) return {changed:false,remoteHash:before,sourceHash:after,published:false};
  if((input.mode??'check')==='check') return {changed:true,remoteHash:before,sourceHash:after,published:false};
  if(!input.expectedHash) throw new Error('Publishing requires the remoteHash from a reviewed check');
  await call('PUT',{collection:input.collection});
  const verified=(await call('GET')).collection;
  validate(verified);
  if(fingerprint(verified)!==after) throw new Error('Postman read-back differs from the submitted collection');
  return {changed:true,remoteHash:before,sourceHash:after,published:true};
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try {
    const file=process.env.POOLSTER_POSTMAN_COLLECTION;
    if(!file) throw new Error('POOLSTER_POSTMAN_COLLECTION is required');
    const bytes=await readFile(file);
    if(bytes.length>10*1024*1024) throw new Error('Collection exceeds 10 MiB');
    const result=await syncCollection({collection:JSON.parse(bytes),uid:process.env.POOLSTER_POSTMAN_UID,token:process.env.POOLSTER_POSTMAN_API_KEY,mode:process.env.POOLSTER_POSTMAN_MODE??'check',expectedHash:process.env.POOLSTER_POSTMAN_EXPECTED_HASH});
    console.log(JSON.stringify(result));
    if(process.env.POOLSTER_POSTMAN_MODE!=='publish' && result.changed) process.exitCode=1;
  } catch(error) {console.error(error instanceof SyntaxError?'Collection is invalid JSON':error.message);process.exitCode=1;}
}
