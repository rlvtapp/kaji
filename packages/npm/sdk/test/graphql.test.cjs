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
test('unsupported GraphQL outputs warn and leave prior files intact', async (t) => {
  const {dir,input} = await fixture(t);
  const output = path.join(dir,'out');
  await fs.mkdir(output); await fs.writeFile(path.join(output,'local.txt'),'keep');
  input.path=path.join(dir,'missing.graphql');
  const result = await generate({input,output,plugins:[bundle.pluginGo()]});
  assert.equal(result.skipped.length,1);
  assert.deepEqual(result.changes,{added:[],modified:[],removed:[]});
  assert.equal(await fs.readFile(path.join(output,'local.txt'),'utf8'),'keep');
});
test('mixed GraphQL generation preserves skipped owned outputs and local edits', async (t) => {
  const {dir,input} = await fixture(t);
  const output=path.join(dir,'out');
  const binding=require(process.env.POOLSTER_NODE_BINARY);
  await binding.materialize(JSON.stringify([{path:'go/client.go',contents:'old',owner:'go-sdk',preserveExisting:false}]),output,true);
  await fs.writeFile(path.join(output,'go/client.go'),'locally edited');
  const result=await generate({input,output,plugins:[bundle.pluginTypeScript(),bundle.pluginGo()]});
  assert.equal(result.skipped.length,1);
  assert.equal(await fs.readFile(path.join(output,'go/client.go'),'utf8'),'locally edited');
  assert.ok(result.files.some(file=>file.path.endsWith('graphql.ts')));
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
  await generate({input,output,plugins:[bundle.pluginTypeScript()]});
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
  assert.deepEqual(await operations.Joined(transport,{}),{kind:'success',data:{joinedAt:'2026-10-09'},extensions:undefined});
});
