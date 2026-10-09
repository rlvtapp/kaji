import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import * as sdk from '@poolster-test/graphql-client';
const require = createRequire(import.meta.url);
const { graphql, buildSchema, version } = require(process.env.POOLSTER_GRAPHQL_JS);
assert.equal(version, '16.14.2');
const schema = buildSchema(process.env.POOLSTER_RELEASE_SCHEMA);
let name = 'Ada';
const requests = [];
const user = id => ({id, name, nickname: null, fragile() { throw new Error('partial failure'); }});
const rootValue = { user: ({id, filter}) => {if (filter) assert.equal(filter.limit, 3);return user(id);}, rename: ({name: value}) => {name = value;return user('1');}, fatal() {throw new Error('fatal failure');} };
const server = createServer(async (request,response) => {
  let body = ''; for await (const chunk of request) body += chunk;
  requests.push({url:request.url, headers:request.headers, body:JSON.parse(body)});
  if (request.url === '/http-error') {response.writeHead(503);response.end('unavailable');return;}
  if (request.url === '/bad-json') {response.end('broken');return;}
  if (request.url === '/bad-envelope') {response.end('{"data":123}');return;}
  const {query, variables, operationName} = JSON.parse(body);
  const result = await graphql({schema, source:query, variableValues:variables, operationName, rootValue});
  response.setHeader('content-type','application/graphql-response+json');response.end(JSON.stringify(result));
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const endpoint = `http://127.0.0.1:${server.address().port}`;
try {
  assert.equal(sdk.releaseMarker, 'customized-release');
  const transport = sdk.createGraphqlHttpTransport(endpoint, {headers:{'x-package':'installed'}});
  const result = await sdk.Read(transport,{id:'7',filter:{prefix:'selected'}});
  assert.equal(result.kind,'success');assert.deepEqual(result.data,{person:{id:'7',name:'Ada',nickname:null}});
  assert.deepEqual(requests[0].body.variables,{id:'7',filter:{prefix:'selected'}});
  assert.equal(requests[0].body.operationName,'Read');assert.equal(requests[0].headers['x-package'],'installed');
  assert.equal((await sdk.Rename(transport,{name:'Grace'})).data.rename.name,'Grace');
  const partial = await sdk.Partial(transport,{id:'7'});
  assert.equal(partial.kind,'partial');assert.equal(partial.data.user.name,'Grace');assert.equal(partial.data.user.fragile,null);
  assert.equal(partial.errors[0].message,'partial failure');assert.deepEqual(partial.errors[0].path,['user','fragile']);
  const failure = await sdk.Fatal(transport,{});assert.equal(failure.kind,'error');assert.equal(failure.data,null);
  assert.equal(failure.errors[0].message,'fatal failure');
  await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(endpoint+'/http-error'),{id:'7'}),sdk.GraphqlHttpError);
  for (const path of ['/bad-json','/bad-envelope']) await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(endpoint+path),{id:'7'}),sdk.GraphqlProtocolError);
  const offline = new Error('transport offline');
  await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(endpoint,{fetch:async()=>{throw offline;}}),{id:'7'}),error=>error===offline);
  const abort = new AbortController();abort.abort();await assert.rejects(sdk.Read(transport,{id:'7'},{signal:abort.signal}),error=>error.name==='AbortError');
  assert.equal((await sdk.read(transport,{id:'7'})).kind,'success');
  const style=process.env.POOLSTER_CLIENT_STYLE;
  if(style==='raw') assert.equal(sdk.createClient,undefined);
  else {
    let fetches=0;
    const client=sdk.createClient({endpoint,headers:{'x-package':'style-configured-once'},fetch:async(...args)=>{fetches++;return fetch(...args);}});
    const query=style==='flat'?client:client.query;
    const read=style==='grouped'?client.user:query;
    const mutation=style==='flat'?client:style==='grouped'?client.user:client.mutation;
    const userResult=await query.readUser({id:'42'});
    assert.equal(userResult.kind,'success');assert.deepEqual(userResult.data.user,{id:'42',name:'Grace',nickname:null});
    assert.equal(requests.at(-1).body.operationName,'ReadUser');assert.deepEqual(requests.at(-1).body.variables,{id:'42'});
    assert.equal((await read.read({id:'7'})).data.person.name,'Grace');
    assert.equal((await mutation.rename({name:'Style'})).data.rename.name,'Style');
    assert.equal((await query.partial({id:'7'})).kind,'partial');
    assert.equal((await query.fatal({})).kind,'error');
    assert.equal(fetches,5);
    assert.ok(requests.slice(-5).every(request=>request.headers['x-package']==='style-configured-once'));
    const cancelled=new AbortController();cancelled.abort();
    await assert.rejects(read.read({id:'7'},{signal:cancelled.signal}),error=>error.name==='AbortError');
  }
  console.log('Installed GraphQL package: variables, selections/nullability, mutation, partial/errors, HTTP/protocol/fetch/abort, customization passed');
} finally { await new Promise(resolve=>server.close(resolve)); }
