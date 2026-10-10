'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { generate, loadConfig } = require('../index.cjs');
const bundle = require('../../plugins-all/index.cjs');
const { temporary } = require('../test-support/helpers.cjs');

async function fixture(t) {
  const dir = await temporary(t);
  await fs.writeFile(path.join(dir, 'schema.graphql'), 'scalar DateTime\ntype Query { joinedAt: DateTime! }');
  await fs.writeFile(path.join(dir, 'query.graphql'), 'query Joined { joinedAt }');
  return { dir, input: { path: path.join(dir, 'schema.graphql'), plugin: bundle.inputGraphql(), operations: [path.join(dir, 'query.graphql')], scalars: { DateTime: { input: 'string', output: 'string' } } } };
}
test('native GraphQL generates through the existing TypeScript package and regenerates', async (t) => {
  const {dir,input} = await fixture(t);
  const configuration = { input, output: path.join(dir,'out'), plugins: [bundle.pluginTypeScript()] };
  const first = await generate(configuration);
  assert.equal(first.api,null);
  assert.deepEqual(first.skipped,[]);
  assert.ok(first.files.some((file) => file.path.endsWith('package.json')));
  assert.ok(first.files.some((file) => /"joinedAt": \(string\)/.test(file.contents)));
  assert.deepEqual((await generate(configuration)).changes,{added:[],modified:[],removed:[]});
});
test('Java GraphQL output generates and preserves unrelated files', async (t) => {
  const {dir,input} = await fixture(t);
  delete input.scalars;
  const output = path.join(dir,'out');
  await fs.mkdir(output); await fs.writeFile(path.join(output,'local.txt'),'keep');
  const config = {input,output,plugins:[bundle.pluginJava()]};
  const result = await generate(config);
  assert.deepEqual(result.skipped,[]);
  assert.ok(result.files.some(file => file.path.endsWith('.java')));
  assert.equal(await fs.readFile(path.join(output,'local.txt'),'utf8'),'keep');
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
});
test('mixed TypeScript and Java GraphQL generation emits both consumers', async (t) => {
  const {dir,input} = await fixture(t);
  delete input.scalars;
  const result=await generate({input,output:path.join(dir,'out'),plugins:[bundle.pluginTypeScript(),bundle.pluginJava()]});
  assert.deepEqual(result.skipped,[]);
  assert.ok(result.files.some(file=>file.path.endsWith('graphql.ts')));
  assert.ok(result.files.some(file=>file.path.endsWith('.java')));
});
test('GraphQL invalid operations fail before output is written', async (t) => {
  const {dir,input} = await fixture(t);
  await fs.writeFile(input.operations[0], 'query Bad { missing }');
  await assert.rejects(generate({input,output:path.join(dir,'out'),plugins:[bundle.pluginTypeScript()]}), /missing|validation/i);
  await assert.rejects(fs.stat(path.join(dir,'out')), {code:'ENOENT'});
});
test('loaded GraphQL config resolves operation files alongside schema', async (t) => {
  const {dir} = await fixture(t);
  const file = path.join(dir,'poolster.config.cjs');
  await fs.writeFile(file, `module.exports={input:{path:'schema.graphql',operations:['query.graphql'],plugin:{kind:'native-input',name:'graphql',format:'graphql',provider:'graphql.apollo'}},output:'out',plugins:[{kind:'native-sdk',name:'ts',package:{language:'typescript',path:'ts'}}]}`);
  const config=await loadConfig(file);
  assert.deepEqual(config.input.operations,[path.join(dir,'query.graphql')]);
});

test('mixed native GraphQL uses independent TypeScript and Rust scalar maps', async (t) => {
  const {dir,input}=await fixture(t);
  input.rustScalars={DateTime:{input:'String',output:'String'}};
  const config={input,output:path.join(dir,'out'),plugins:[bundle.pluginTypeScript(),bundle.pluginRust({name:'graphql_client'})]};
  const result=await generate(config);
  assert.ok(result.files.some(file=>file.path.endsWith('Cargo.toml')));
  assert.ok(result.files.some(file=>/pub joined_at: std::string::String/.test(file.contents)));
  assert.ok(result.files.some(file=>/"joinedAt": \(string\)/.test(file.contents)));
  assert.deepEqual(result.skipped,[]);
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
  input.rustScalars.DateTime.output='string';
  await assert.rejects(generate(config), /scalar|Rust|unsupported/i);
  input.rustScalars.DateTime.output='String';
  input.rustScalars.DateTime.extra='invalid';
  await assert.rejects(generate(config), /unknown field/i);
});
test('npm GraphQL package compiles and executes against a local GraphQL server', async (t) => {
  const dependencies = process.env.POOLSTER_GRAPHQL_TEST_NODE_MODULES;
  if (!dependencies) return t.skip('set POOLSTER_GRAPHQL_TEST_NODE_MODULES to pinned TypeScript/GraphQL dependencies');
  const { execFileAsync } = require('../test-support/helpers.cjs');
  const http = require('node:http');
  const { pathToFileURL } = require('node:url');
  const { buildSchema, graphql } = require(path.join(dependencies, 'graphql'));
  const {dir,input} = await fixture(t);
  const output = path.join(dir,'out');
  await fs.writeFile(input.operations[0], 'query ReadUser { joinedAt }');
  await generate({input,output,plugins:[bundle.pluginTypeScript({contracts:{graphql:{style:'flat',scalars:input.scalars}}})]});
  const target = path.join(output,'typescript');
  await execFileAsync(process.execPath,[path.join(dependencies,'typescript/lib/tsc.js'),'--project',path.join(target,'tsconfig.json')]);
  const schema=buildSchema('scalar DateTime\ntype Query { joinedAt: DateTime! }');
  const server=http.createServer(async(req,res)=>{
    let body=''; for await(const chunk of req) body+=chunk;
    const request=JSON.parse(body);
    const result=await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:{joinedAt:()=> '2026-10-09'}});
    res.setHeader('content-type','application/json');res.end(JSON.stringify(result));
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  t.after(()=>new Promise(resolve=>server.close(resolve)));
  const operations=await import(pathToFileURL(path.join(target,'dist/graphql.js')));
  const runtime=await import(pathToFileURL(path.join(target,'dist/graphql-runtime.js')));
  const transport=runtime.createGraphqlHttpTransport(`http://127.0.0.1:${server.address().port}`);
  assert.deepEqual(await operations.ReadUser(transport,{}),{kind:'success',data:{joinedAt:'2026-10-09'},extensions:undefined});
  const bound=operations.createClient({transport});
  assert.deepEqual(await bound.readUser({}),{kind:'success',data:{joinedAt:'2026-10-09'},extensions:undefined});
});

test('GraphQL output plugin options select styles and reject conflicting aliases', async (t) => {
  const {dir,input}=await fixture(t);
  for (const style of ['raw','flat','idiomatic','namespaced']) {
    const configuration={input,output:path.join(dir,style),plugins:[bundle.pluginTypeScript({style,scalars:input.scalars}),bundle.pluginRust({name:'graphql_client',style,scalars:{DateTime:{input:'String',output:'String'}}})]};
    const result=await generate(configuration);
    const ts=result.files.filter(file=>file.path.startsWith('typescript/') && file.path.endsWith('.ts')).map(file=>file.contents).join('\n');
    assert.equal(ts.includes('export function createClient'),style !== 'raw');
    assert.deepEqual((await generate(configuration)).changes,{added:[],modified:[],removed:[]});
  }
  const config=(options)=>({input,output:path.join(dir,'invalid'),plugins:[bundle.pluginTypeScript(options)]});
  await assert.rejects(generate(config({raw:true,style:'flat'})),/mutually exclusive/);
  await assert.rejects(generate(config({scalars:{DateTime:{input:'unknown',output:'unknown'}}})),/conflicting TypeScript scalar mapping/);
  await assert.rejects(generate(config({style:'flat',groups:{user:{read:'Joined'}}})),/groups|idiomatic/i);
  const grouped=await generate(config({style:'idiomatic',groups:{user:{read:'Joined'}}}));
  assert.ok(grouped.files.some(file=>file.contents.includes('user') && file.contents.includes('read')));
});

test('contract-scoped exporter options are independent and diagnose conflicts', async (t) => {
  const {dir,input}=await fixture(t);
  delete input.scalars;
  const plugin=(extra={})=>bundle.pluginTypeScript({contracts:{http:{style:'flat'},graphql:{style:'grouped',scalars:{DateTime:{input:'string',output:'string'}},groups:{user:{read:'Joined'}}}},...extra});
  const config=(p)=>({input,output:path.join(dir,'scoped'),plugins:[p]});
  const result=await generate(config(plugin()));
  assert.ok(result.files.some(file=>file.contents.includes('user') && file.contents.includes('read')));
  assert.deepEqual((await generate(config(plugin()))).changes,{added:[],modified:[],removed:[]});
  await assert.rejects(generate(config(plugin({style:'flat'}))),/conflicting.*style/);
  await assert.rejects(generate(config(bundle.pluginTypeScript({contracts:{unknown:{style:'flat'}}}))),/unsupported bundled exporter contract|unknown field/);
});

test('native GraphQL entry point composes existing ecosystem packages and mutation options', async (t) => {
  const {dir,input}=await fixture(t);
  delete input.scalars;
  await fs.writeFile(input.path,'type User { id: ID!, name: String! } type Query { readUser(id: ID!): User! } type Mutation { renameUser(id: ID!, name: String!): User! }');
  await fs.writeFile(input.operations[0],'query ReadUser($id: ID!) { readUser(id:$id) { id name } } mutation RenameUser($id: ID!, $name: String!) { renameUser(id:$id,name:$name) { id name } }');
  const plugins=[bundle.pluginTypeScript({contracts:{graphql:{style:'flat'}}}),bundle.pluginReactQuery(),bundle.pluginVueQuery(),bundle.pluginSwr(),bundle.pluginZod(),bundle.pluginFaker({fixtureOptions:{seed:42}}),bundle.pluginMsw(),bundle.pluginCypress({cypressOptions:{baseUrl:'http://localhost:4000/graphql',includeMutations:true,timeoutMs:5000}})];
  const config={input,output:path.join(dir,'ecosystem'),plugins};
  const result=await generate(config);
  for(const name of ['react-query','vue-query','swr','zod','faker','msw','cypress']) assert.ok(result.files.some(file=>file.path.includes(name)),name);
  assert.ok(result.files.some(file=>file.path.includes('cypress')&&file.contents.includes('RenameUser')));
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
  if (process.env.POOLSTER_GRAPHQL_ADDON_NODE_MODULES) {
    const target=path.join(config.output,'typescript');
    await fs.symlink(process.env.POOLSTER_GRAPHQL_ADDON_NODE_MODULES,path.join(target,'node_modules'),'dir');
    const {execFileAsync}=require('../test-support/helpers.cjs');
    await execFileAsync(process.execPath,[path.join(process.env.POOLSTER_GRAPHQL_TEST_NODE_MODULES,'typescript/lib/tsc.js'),'--project',path.join(target,'tsconfig.json')]);
  }
  const unsupported={...config,output:path.join(dir,'unsupported'),plugins:[bundle.pluginTypeScript(),bundle.pluginCypress({cypressOptions:{operationOverrides:{ReadUser:{path:'/http-only'}}}})]};
  await assert.rejects(generate(unsupported),/operation.overrides|HTTP|unsupported/i);
});

test('HTTP selects its own contract options without applying GraphQL configuration', async (t) => {
  const {artifacts,config}=require('../test-support/helpers.cjs');
  const dir=await temporary(t);
  const compiled=await artifacts(t);
  const input={artifacts:compiled};
  const baseline=await generate(config(input,path.join(dir,'http-flat'),[bundle.pluginTypeScript({style:'flat'})]),{write:false});
  const scoped=await generate(config(input,path.join(dir,'http-scoped'),[bundle.pluginTypeScript({contracts:{http:{style:'flat'},graphql:{style:'grouped',groups:{user:{read:'NotAnHttpOperation'}},scalars:{DateTime:{input:'string',output:'string'}}}}})]),{write:false});
  assert.deepEqual(scoped.files,baseline.files);
  await assert.rejects(generate(config(input,path.join(dir,'http-bad'),[bundle.pluginTypeScript({scalars:{DateTime:{input:'string',output:'string'}}})])),/require GraphQL input/);
});
test('GraphQL Go and Python packages use existing factories and regenerate without HTTP lowering', async (t) => {
  const { dir, input } = await fixture(t);
  const configuration = { input, output: path.join(dir, 'native-languages'), plugins: [
    bundle.pluginGo({ contracts: { graphql: { style: 'flat' } } }),
    bundle.pluginPython({ contracts: { graphql: { style: 'flat' } } }),
  ] };
  const first = await generate(configuration);
  assert.equal(first.api, null);
  assert.deepEqual(first.skipped, []);
  assert.ok(first.files.some(f => f.path.includes('graphql_operation_')));
  assert.ok(first.files.some(f => f.path.endsWith('.py')));
  assert.deepEqual((await generate(configuration)).changes, { added: [], modified: [], removed: [] });
  configuration.plugins = [bundle.pluginPython({ contracts: { graphql: { scalars: { DateTime: { input: 'str', output: 'str' } } } } })];
  await assert.rejects(generate(configuration, { write: false }), /custom scalar mappings/);
});

test('Other language GraphQL factories share the native input and regenerate', async (t) => {
  const {dir,input} = await fixture(t);
  delete input.scalars;
  const configuration = {input, output:path.join(dir,'other-languages'), plugins:[
    bundle.pluginPhp({contracts:{graphql:{style:'raw'}}}),
    bundle.pluginJava({contracts:{graphql:{style:'flat'}}}),
    bundle.pluginCSharp({contracts:{graphql:{style:'grouped',groups:{users:{read:'Joined'}}}}}),
    bundle.pluginRuby({contracts:{graphql:{style:'flat'}}}),
    bundle.pluginSwift({contracts:{graphql:{style:'flat'}}}),
    bundle.pluginElixir({contracts:{graphql:{style:'grouped'}}}),
  ]};
  const result = await generate(configuration);
  assert.deepEqual(result.skipped,[]);
  for (const ext of ['.php','.java','.cs','.rb','.swift','.ex']) assert.ok(result.files.some(file => file.path.endsWith(ext)),ext);
  assert.deepEqual((await generate(configuration)).changes,{added:[],modified:[],removed:[]});
});

test('GraphQL collection and CLI factories generate native packages and regenerate', async (t) => {
  const {dir,input} = await fixture(t); delete input.scalars;
  const tools = require('../plugins.cjs');
  const configuration = {input, output:path.join(dir,'out'), plugins:[
    tools.pluginGraphqlPostman({endpoint:'http://localhost:4000/graphql'}),
    tools.pluginGraphqlRustCli({commandName:'users'}),
    tools.pluginGraphqlTypeScriptCli({commandName:'users'}),
  ]};
  const result = await generate(configuration);
  assert.deepEqual(result.skipped,[]);
  assert.ok(result.files.some(f=>f.path==='postman/collection.json'));
  assert.ok(result.files.some(f=>f.path==='postman/environment.json'));
  assert.ok(result.files.some(f=>f.path==='rust-cli/Cargo.toml'));
  assert.ok(result.files.some(f=>f.path==='typescript-cli/package.json'));
  assert.deepEqual((await generate(configuration)).changes,{added:[],modified:[],removed:[]});
});

test('GraphQL advanced input resolves imports and dispatches ten streaming SDKs', async(t)=>{
 const {dir,input}=await fixture(t);delete input.scalars;
 await fs.mkdir(path.join(dir,'imports'));
 await fs.writeFile(input.path,'#import "types.graphql"\ndirective @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT type Query{user:User!} type Subscription{ticks:User!}');
 await fs.writeFile(path.join(dir,'imports/types.graphql'),'type User{id:ID! name:String!}');
 input.importRoots=[path.join(dir,'imports')];
 const methods=['TypeScript','Rust','Go','Python','Php','Java','CSharp','Ruby','Swift','Elixir'];
 for(const incremental of [false,true]){
  input.incremental=incremental;input.subscriptions=false;
  await fs.writeFile(input.operations[0],incremental?'query Read{user{id ... @defer(label:"details"){name}}}':'query Read{user{id name}} subscription Ticks{ticks{id name}}');
  const plugins=methods.map(language=>bundle[`plugin${language}`]({name:language==='Php'?'example/graphql-client':'graphql_client',contracts:{graphql:{style:'raw',subscriptions:!incremental}}}));
  const config={input,output:path.join(dir,incremental?'incremental':'subscriptions'),plugins};
  const result=await generate(config);assert.deepEqual(result.skipped,[]);
  for(const language of methods){assert.ok(result.files.some(f=>f.path.startsWith(language.toLowerCase()+'/')),language);}
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
 }
});

test('Symfony GraphQL generates portable SDK and bundle with deterministic regeneration', async (t) => {
  const {dir,input}=await fixture(t); delete input.scalars;
  const config={input,output:path.join(dir,'out'),plugins:[{kind:'native-sdk',name:'symfony',package:{language:'symfony',path:'bundle',name:'acme/graphql',contracts:{graphql:{style:'flat'}}}}]};
  const result=await generate(config);
  assert.deepEqual(result.skipped,[]);
  assert.ok(result.files.some(f=>f.path.endsWith('src/Symfony/AcmeGraphqlBundle.php')));
  assert.ok(result.files.some(f=>f.path.endsWith('src/Client.php')));
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
  config.plugins[0].package.contracts.graphql.subscriptions=true;
  await assert.rejects(generate(config),/Symfony GraphQL subscriptions are unsupported/);
  delete config.plugins[0].package.contracts.graphql.subscriptions;
  input.incremental=true;
  assert.equal((await generate(config)).skipped.length,1);
});
