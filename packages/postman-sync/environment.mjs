import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';

// Only portable environment fields are submitted. Service metadata never enters PUT.
export function portableEnvironment(value) {
  if (!value || typeof value.name !== 'string' || !value.name.trim() || !Array.isArray(value.values)) throw new Error('Expected a Postman environment document');
  const keys = new Set();
  const values = value.values.map(variable => {
    if (!variable || typeof variable.key !== 'string' || !variable.key || keys.has(variable.key) || typeof variable.value !== 'string' || !['default','secret'].includes(variable.type ?? 'default') || (variable.enabled !== undefined && typeof variable.enabled !== 'boolean')) throw new Error('Invalid or duplicate environment variable');
    keys.add(variable.key);
    return {key:variable.key,value:variable.value,type:variable.type ?? 'default',enabled:variable.enabled ?? true};
  });
  if (Buffer.byteLength(JSON.stringify(value)) > 10*1024*1024) throw new Error('Environment exceeds 10 MiB');
  return {name:value.name,values};
}
export function environmentFingerprint(value) {
  const portable=portableEnvironment(value);
  portable.values.sort((a,b)=>a.key.localeCompare(b.key));
  return createHash('sha256').update(JSON.stringify(portable)).digest('hex');
}
export function mergeEnvironment(source,remote) {
  source=portableEnvironment(source);remote=portableEnvironment(remote);
  const values=new Map(remote.values.map(value=>[value.key,value]));
  for (const variable of source.values) {
    const previous=values.get(variable.key);
    // Generated secret placeholders cannot erase or upload shared credentials.
    if (variable.type==='secret' && variable.value!=='') throw new Error('Source secret values must be empty placeholders');
    if (variable.type==='secret' || previous?.type==='secret') {
      if (variable.value!=='') throw new Error('Source cannot replace a remote secret variable');
      values.set(variable.key,{...variable,type:'secret',value:previous?.value ?? ''});
    } else values.set(variable.key,variable);
  }
  return {name:source.name,values:[...values.values()]};
}
export async function syncEnvironment(input,{fetch:request=globalThis.fetch}={}) {
  portableEnvironment(input.environment);
  if (!/^[a-zA-Z0-9_-]+$/.test(input.uid??'')) throw new Error('An explicit destination environment UID is required');
  if (!input.token || /[\r\n]/.test(input.token)) throw new Error('Postman API token is required');
  const mode=input.mode??'check';
  if (!['check','publish'].includes(mode)) throw new Error('Mode must be check or publish');
  const url=`https://api.getpostman.com/environments/${input.uid}`;
  async function call(method,body) {
    let response;
    try {response=await request(url,{method,headers:{'X-Api-Key':input.token,'Content-Type':'application/json'},body:body&&JSON.stringify(body),redirect:'error',signal:AbortSignal.timeout(30000)});} catch {throw new Error('Postman API request failed');}
    if (!response.ok) throw new Error(`Postman API returned HTTP ${response.status}`);
    let bytes=0,text='';
    try {
      // Bound reads while streaming, rather than after allocating an arbitrary response.
      if (response.body) {
        const reader=response.body.getReader(), decoder=new TextDecoder();
        try {for (;;) {const {value,done}=await reader.read();if(done)break;bytes+=value.byteLength;if(bytes>12*1024*1024){await reader.cancel();throw new Error('Postman response exceeds 12 MiB');}text+=decoder.decode(value,{stream:true});}text+=decoder.decode();} finally {reader.releaseLock();}
      } else text=await response.text();
    } catch(error) {if(error.message==='Postman response exceeds 12 MiB')throw error;throw new Error('Postman API response failed');}
    if(Buffer.byteLength(text)>12*1024*1024)throw new Error('Postman response exceeds 12 MiB');
    try {return JSON.parse(text);} catch {throw new Error('Postman API returned invalid JSON');}
  }
  const remote=(await call('GET')).environment;
  const desired=mergeEnvironment(input.environment,remote);
  const before=environmentFingerprint(remote),after=environmentFingerprint(desired);
  if(input.expectedHash && (!/^[a-f0-9]{64}$/.test(input.expectedHash)||input.expectedHash!==before))throw new Error('Remote environment changed; review it before publishing');
  if(before===after)return {changed:false,remoteHash:before,sourceHash:after,published:false};
  if(mode==='check')return {changed:true,remoteHash:before,sourceHash:after,published:false};
  if(!input.expectedHash)throw new Error('Publishing requires the remoteHash from a reviewed check');
  await call('PUT',{environment:desired});
  const verified=(await call('GET')).environment;
  if(environmentFingerprint(verified)!==after)throw new Error('Postman read-back differs from the submitted environment');
  return {changed:true,remoteHash:before,sourceHash:after,published:true};
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try {
    if(!process.env.KAJI_POSTMAN_ENVIRONMENT)throw new Error('KAJI_POSTMAN_ENVIRONMENT is required');
    const bytes=await readFile(process.env.KAJI_POSTMAN_ENVIRONMENT);
    if(bytes.length>10*1024*1024)throw new Error('Environment exceeds 10 MiB');
    const result=await syncEnvironment({environment:JSON.parse(bytes),uid:process.env.KAJI_POSTMAN_UID,token:process.env.KAJI_POSTMAN_API_KEY,mode:process.env.KAJI_POSTMAN_MODE??'check',expectedHash:process.env.KAJI_POSTMAN_EXPECTED_HASH});
    console.log(JSON.stringify(result));
    if(process.env.KAJI_POSTMAN_MODE!=='publish' && result.changed)process.exitCode=1;
  } catch(error) {console.error(error instanceof SyntaxError?'Environment is invalid JSON':error.message);process.exitCode=1;}
}
