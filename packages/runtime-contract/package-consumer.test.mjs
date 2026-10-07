/** Build, pack, install, import and type-check the package a customer receives. */
import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,cp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {execute} from './runner.mjs';

const fixture=process.env.KAJI_PACKAGE_FIXTURE;
const compiler=process.env.KAJI_TSC_JS;
test('installed TypeScript ESM package exposes executable root/subpath exports and customer types', {skip:!fixture||!compiler}, async t=>{
  const root=await mkdtemp(join(tmpdir(),'kaji-installed-consumer-'));
  t.after(()=>rm(root,{recursive:true,force:true}));
  const sdk=join(root,'sdk');await cp(resolve(fixture),sdk,{recursive:true});
  await execute('node',[compiler,'-p','tsconfig.json'],{cwd:sdk});
  const archives=join(root,'archives');await mkdir(archives);
  const packed=JSON.parse(await execute('npm',['pack','--json','--ignore-scripts','--pack-destination',archives],{cwd:sdk,env:{npm_config_cache:join(root,'npm-cache')}}));
  assert.equal(packed.length,1);
  assert(packed[0].files.some(file=>file.path==='dist/index.js'));
  assert(packed[0].files.some(file=>file.path==='dist/index.d.ts'));
  const consumer=join(root,'consumer');await mkdir(consumer);
  await writeFile(join(consumer,'package.json'),JSON.stringify({name:'kaji-local-consumer',private:true,type:'module'}));
  await execute('npm',['install','--offline','--ignore-scripts','--omit=dev','--no-audit','--no-fund',join(archives,packed[0].filename)],{cwd:consumer,env:{npm_config_cache:join(root,'npm-cache')}});
  const {name}=JSON.parse(await readFile(join(sdk,'package.json'),'utf8'));
  const imports=`import {KajiContract} from ${JSON.stringify(name)};\nimport {createContact} from ${JSON.stringify(`${name}/clients/contacts/createContact`)};\n`;
  await writeFile(join(consumer,'probe.mjs'),imports+`
import assert from 'node:assert/strict';
let calls=0;
const client=new KajiContract({baseUrl:'https://unused.test',auth:{Bearer:'test-token'},retry:false,fetch:async(url,request)=>{
  calls++;assert.equal(request.headers.get('authorization'),'Bearer test-token');
  return new Response(JSON.stringify({id:'installed'}),{status:200,headers:{'content-type':'application/json'}});
}});
const model=await client.contacts.get({throwOnError:true});assert.equal(model.id,'installed');assert.equal(calls,1);
const created=await createContact({client:async request=>{
 assert.match(request.headers['X-Once'],/^[0-9a-f-]{36}$/);
 return {status:200,data:{id:'subpath'},headers:{},contentType:'application/json'};
},throwOnError:true});assert.equal(created.id,'subpath');
`);
  await execute('node',['probe.mjs'],{cwd:consumer});
  await writeFile(join(consumer,'probe.mts'),imports+`
const client=new KajiContract({baseUrl:'https://unused.test'});
const result=await client.contacts.get({throwOnError:true});
const id:string=result.id;
void id; void createContact;
`);
  await execute('node',[compiler,'--strict','--noEmit','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--lib','ES2022,DOM,DOM.Iterable','probe.mts'],{cwd:consumer});
});
