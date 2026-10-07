#!/usr/bin/env node
import {spawnSync} from 'node:child_process';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

export function deliveryPlan({mode='local', binary, repository, registry}={}) {
  if (!['local','mock','live'].includes(mode)) throw Error('mode must be local, mock, or live');
  if (mode === 'live' && (!/^[A-Za-z0-9-]+\/[A-Za-z0-9_.-]+$/.test(repository ?? '') || !registry)) throw Error('live requires explicit disposable repository and registry');
  return {mode, binary:binary ?? resolve('target/debug/kaji'), repository, registry,
    remoteWrites:false, publication:false};
}

// Live currently verifies authenticated read-only observations. Publication is
// deliberately not delegated to this harness: run reviewed registry workflows.
export function execute(plan, run=spawnSync) {
  const root = mkdtempSync(join(tmpdir(), 'kaji-delivery-'));
  const calls=[];
  const invoke=(args,json=false)=>{
    calls.push(args);
    const result=run(plan.binary,args,{cwd:root,encoding:'utf8',env:{...process.env}});
    if (result.status !== 0) throw Error(`delivery step failed: ${args[0]} ${args[1] ?? ''}`);
    if (!json) return result.stdout;
    const text=result.stdout.trim();
    const start=text.search(/(?:^|\n)[{[]/);
    if(start<0) throw Error('delivery step produced no JSON report');
    return JSON.parse(text.slice(start).trim());
  };
  try {
    writeFileSync(join(root,'api.json'),JSON.stringify({openapi:'3.0.3',info:{title:'Delivery Probe',version:'1.0.0'},paths:{'/probe':{get:{operationId:'getProbe',responses:{'200':{description:'Probe',content:{'application/json':{schema:{type:'string'}}}}}}}}}));
    writeFileSync(join(root,'kaji.json'),JSON.stringify({openapi:{input:'api.json'},output:{path:'generated'},packages:[{language:'typescript',path:'typescript',name:'@kaji-delivery/probe',version:'0.1.0',plugins:[{name:'sdk',transport:'fetch'}],release:{build:[{program:'node',args:['--version']}],test:[{program:'node',args:['--version']}],publisher:{registry:'npm',release_type:'node'}}}]}));
    invoke(['generate','--config','kaji.json']);
    const drift=invoke(['generate','--config','kaji.json','--check','--format','json'],true);
    if (!['added','modified','removed'].every(key=>Array.isArray(drift[key]) && drift[key].length===0)) throw Error('generated SDK drift is not empty');
    invoke(['sdk','doctor','--root','generated','--json'],true);
    invoke(['sdk','inspect','--root','generated','--json'],true);
    invoke(['sdk','run','--root','generated','--package','typescript','--phase','build']);
    invoke(['sdk','run','--root','generated','--package','typescript','--phase','test']);
    invoke(['sdk','init','--root','generated','--dry-run']);
    if(plan.mode==='live') invoke(['sdk','status','--remote','--repository',plan.repository,'--json'],true);
    return {mode:plan.mode,steps:calls.length,drift,remoteWrites:false,publication:false,registry:plan.registry ?? null};
  } finally {rmSync(root,{recursive:true,force:true});}
}
if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const args=process.argv.slice(2); const option=name=>{const i=args.indexOf(name);return i<0?undefined:args[i+1];};
  console.log(JSON.stringify(execute(deliveryPlan({mode:option('--mode'),binary:option('--binary'),repository:option('--repository'),registry:option('--registry')})),null,2));
}
