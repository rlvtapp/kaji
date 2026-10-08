const { createServer } = require('node:http');
const assert = require('node:assert/strict');
const { graphql, buildSchema, version } = require(process.env.POOLSTER_GRAPHQL_JS);
assert.equal(version, '16.14.2');
const schema = buildSchema('type User { id: ID! name: String! fragile: String } type Query { user(id: ID!): User! fatal: String! node: Node! } type Mutation { rename(name: String!): User! } interface Node { id: ID! } type Named implements Node { id: ID! label: String! }');
let currentName = 'Ada';
const user = id => ({id, name: currentName, fragile() { throw new Error('partial failure'); }});
const rootValue = { node: () => ({__typename:'Named', id:'n', label:'selected'}), user: ({id}) => user(id), rename: ({name}) => {currentName=name;return user('1');}, fatal() {throw new Error('fatal failure');} };
const server = createServer(async (req,res) => {
 let body=''; for await(const chunk of req) body+=chunk;
 if(req.url === '/http-error') {res.writeHead(503);res.end('unavailable');return;}
 if(req.url === '/bad-json') {res.end('oops');return;}
 if(req.url === '/primitive-data') {res.end('{\"data\":123}');return;}
 if(req.url === '/array-data') {res.end('{\"data\":[]}');return;}
 if(req.url === '/bad-extensions') {res.end('{\"data\":{},\"extensions\":[]}');return;}
 if(req.url === '/bad-envelope') {res.end('{}');return;}
 const {query,operationName,variables}=JSON.parse(body);
 const result=await graphql({schema,source:query,operationName,variableValues:variables,rootValue});
 res.setHeader('Content-Type','application/graphql-response+json');res.end(JSON.stringify(result));
});
(async()=>{
 const sdk=await import('./dist/index.js');
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const base=`http://127.0.0.1:${server.address().port}`;
 try {
 const transport=sdk.createGraphqlHttpTransport(base);
 const anonymous=await import('../anonymous/sdk/dist/index.js');const anonymousResult=await anonymous.Anonymous(transport,{});assert.equal(anonymousResult.kind,'success');assert.equal(anonymousResult.data.user.name,'Ada');
 const result=await sdk.Read(transport,{id:'7'});assert.deepEqual(result,{kind:'success',data:{person:{id:'7',name:'Ada'}},extensions:undefined});
 const mutation=await sdk.Rename(transport,{name:'Grace'});assert.equal(mutation.data.rename.name,'Grace');
 const partial=await sdk.Partial(transport,{id:'7'});assert.equal(partial.kind,'partial');assert.equal(partial.data.user.name,'Grace');assert.equal(partial.data.user.fragile,null);assert.equal(partial.errors[0].message,'partial failure');
 const included=await sdk.Abstract(transport,{include:true});assert.equal(included.data.node.label,'selected');assert.equal(included.data.node.__typename,'Named');
 const excluded=await sdk.Abstract(transport,{include:false});assert.equal(Object.hasOwn(excluded.data.node,'label'),false);
 const failure=await sdk.Fatal(transport,{});assert.equal(failure.kind,'error');assert.equal(failure.data,null);assert.equal(failure.errors[0].message,'fatal failure');
 await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(base+'/http-error'),{id:'1'}),sdk.GraphqlHttpError);
 await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(base+'/bad-json'),{id:'1'}),sdk.GraphqlProtocolError);
 await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(base+'/bad-envelope'),{id:'1'}),sdk.GraphqlProtocolError);
 for(const path of ['/primitive-data','/array-data','/bad-extensions']) await assert.rejects(sdk.Read(sdk.createGraphqlHttpTransport(base+path),{id:'1'}),sdk.GraphqlProtocolError);
 const abort=new AbortController();abort.abort();await assert.rejects(sdk.Read(transport,{id:'1'},{signal:abort.signal}));
 const events=[];for await(const event of sdk.Changed({async *subscribe(document,name,variables) {assert.equal(name,'Changed');assert.match(document,/subscription Changed/);yield {kind:'success',data:{changed:{name:'stream'}}};}},{})) events.push(event);assert.equal(events[0].data.changed.name,'stream');
 console.log('GraphQL generated package runtime passed');
 } finally {await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
