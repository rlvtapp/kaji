import test from 'node:test';
import assert from 'node:assert/strict';
import {deliveryPlan,execute} from '../delivery.mjs';
test('local mock covers generation drift native checks and scaffold without external writes',()=>{
  const calls=[];
  const result=execute(deliveryPlan({mode:'mock',binary:'mock-kaji'}),(program,args)=>{calls.push(args);return {status:0,stdout:'{"added":[],"modified":[],"removed":[]}'};});
  assert.equal(result.steps,7);assert.equal(result.publication,false);
  assert(!calls.some(args=>args.includes('pr') || args.includes('install') || args.includes('publish')));
  assert(calls.some(args=>args.includes('--check')));
});
test('failures stop delivery and live requires explicit test destinations',()=>{
  let calls=0;assert.throws(()=>execute(deliveryPlan({mode:'mock'}),()=>{calls++;return {status:1};}),/step failed/);assert.equal(calls,1);
  assert.throws(()=>deliveryPlan({mode:'live'}),/explicit/);
  const callsSeen=[];execute(deliveryPlan({mode:'live',repository:'test/sdk',registry:'test-registry'}),(_,args)=>{callsSeen.push(args);return {status:0,stdout:'{"added":[],"modified":[],"removed":[]}'};});
  assert(callsSeen.some(args=>args.includes('--remote')));
  assert(!callsSeen.some(args=>args.includes('publish')));
});

test('local release tag feeds checked package into mocked exact-artifact publisher',async t=>{
  const {mkdtemp,mkdir,writeFile,rm,realpath}=await import('node:fs/promises');
  const {tmpdir}=await import('node:os'); const {join}=await import('node:path');
  const {spawnSync}=await import('node:child_process'); const {createHash}=await import('node:crypto');
  const check=await import('../../sdk-check/check.mjs');
  const publish=await import('../../sdk-publish/publish.mjs');
  const root=await realpath(await mkdtemp(join(tmpdir(),'kaji-release-e2e-')));t.after(()=>rm(root,{recursive:true,force:true}));
  const sdk=join(root,'sdk');await mkdir(join(sdk,'.kaji'),{recursive:true});
  const metadata={schema_version:1,name:'@kaji-delivery/probe',language:'typescript',version:'1.2.3',build:[{program:'node',args:['--version']}],test:[{program:'node',args:['--version']}],publisher:{registry:'npm',release_type:'node',commands:[]}};
  await writeFile(join(sdk,'.kaji/package.json'),JSON.stringify(metadata));
  await writeFile(join(sdk,'package.json'),JSON.stringify({name:metadata.name,version:metadata.version}));
  const git=args=>{const r=spawnSync('git',args,{cwd:root,encoding:'utf8'});assert.equal(r.status,0,r.stderr);return r.stdout.trim();};
  git(['init']);git(['config','user.name','Delivery test']);git(['config','user.email','delivery@example.invalid']);git(['add','.']);git(['commit','-m','release fixture']);git(['tag','sdk-v1.2.3']);
  check.execute(check.plan(sdk,'typescript'),sdk);
  const bytes=Buffer.from('bounded mock package archive');const integrity=`sha512-${createHash('sha512').update(bytes).digest('base64')}`;
  let published=false;let uploads=0;
  const dependencies={sleep:async()=>{},run:async(program,args)=>{
    if(program==='git')return git(args);
    assert.equal(program,'npm');
    if(args[0]==='pack'){const destination=args[args.indexOf('--pack-destination')+1];await writeFile(join(destination,'probe.tgz'),bytes);return JSON.stringify([{name:metadata.name,version:metadata.version,filename:'probe.tgz',integrity,files:[]}]);}
    assert.equal(args[0],'publish');published=true;uploads++;return '';
  },fetch:async()=>({status:published?200:404,json:async()=>({name:metadata.name,version:metadata.version,dist:{integrity}})})};
  const plan=await publish.preparePublish({registry:'npm',path:'sdk',workspace:root,tag:'sdk-v1.2.3',temporaryRoot:root},dependencies);
  await publish.publishPrepared(plan,dependencies);await publish.confirmPublication(plan,dependencies);
  assert.equal(uploads,1);
  await assert.rejects(publish.preparePublish({registry:'npm',path:'sdk',workspace:root,tag:'sdk-v9.9.9'},dependencies),/tag does not match/);
});

test('manual workflow plan fails closed on repositories prefixes mutable refs and package identities',async()=>{
  const {validateInput,validateRecipe}=await import('../validate.mjs');
  const input={source:'test/source',destination:'test/sdk',tag:'v1.2.3',version:'0.5.0',registry:'npm',prefix:'@kaji-test/',mode:'preview',language:'typescript',config:'kaji.json'};
  const allowed={repositories:'["test/source","test/sdk"]',packagePrefixes:'["@kaji-test/"]'};
  assert.equal(validateInput(input,allowed).publication,false);
  assert.throws(()=>validateInput({...input,destination:'production/sdk'},allowed),/allowlisted/);
  assert.throws(()=>validateInput({...input,tag:'../main'},allowed),/immutable/);
  assert.throws(()=>validateInput({...input,prefix:'@production/'},allowed),/allowlisted/);
  assert.throws(()=>validateInput({...input,version:'latest'},allowed),/exact/);
  const recipe={output:{path:'generated'},packages:[{language:'typescript',path:'ts',name:'@kaji-test/sdk',release:{publisher:{registry:'npm'}}}]};
  assert.equal(validateRecipe(recipe,input).packages.length,1);
  recipe.packages[0].name='@production/sdk';assert.throws(()=>validateRecipe(recipe,input),/prefix/);
});

test('every failed delivery phase stops subsequent checks and cleans its checkout', async t => {
  const {existsSync, readFileSync} = await import('node:fs');
  for (let failedStep=0; failedStep<7; failedStep++) {
    await t.test(`failure at phase ${failedStep+1}`, () => {
      const calls=[];let temporaryRoot;
      assert.throws(() => execute(deliveryPlan({mode:'mock'}), (program,args,options) => {
        temporaryRoot=options.cwd;
        assert(existsSync(temporaryRoot));
        const recipe=JSON.parse(readFileSync(`${temporaryRoot}/kaji.json`,'utf8'));
        assert.equal(recipe.packages[0].release.publisher.registry,'npm');
        assert.equal(options.encoding,'utf8');
        calls.push(args);
        return {status:calls.length-1===failedStep?1:0,stdout:'{"added":[],"modified":[],"removed":[]}'};
      }), /delivery step failed/);
      assert.equal(calls.length,failedStep+1);
      assert(!existsSync(temporaryRoot), 'failed checkout must be removed');
      assert(!calls.some(args=>args.includes('publish')||args.includes('pr')||args.includes('install')));
    });
  }
});

test('nonempty drift and malformed reports cannot reach builds or delivery setup', async t => {
  const {existsSync}=await import('node:fs');
  for (const report of [
    '{"added":["untracked.ts"],"modified":[],"removed":[]}',
    '{"added":[],"modified":["client.ts"],"removed":[]}',
    '{"added":[],"modified":[],"removed":["old.ts"]}',
    '{"added":[],"modified":[]}',
    'not a JSON report',
    '{broken'
  ]) {
    let calls=0;let root;
    assert.throws(()=>execute(deliveryPlan({mode:'mock'}),(_,args,options)=>{
      root=options.cwd;calls++;
      return {status:0,stdout:calls===2?report:''};
    }));
    assert.equal(calls,2);assert(!existsSync(root));
  }
  await t.test('valid reports and completed preview also clean temporary sources',()=>{
    let root;const result=execute(deliveryPlan({mode:'mock'}),(_,args,options)=>{
      root=options.cwd;return {status:0,stdout:'{"added":[],"modified":[],"removed":[]}'};
    });
    assert.equal(result.steps,7);assert(!existsSync(root));
  });
});
