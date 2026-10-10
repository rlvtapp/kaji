import assert from 'node:assert/strict';
import http from 'node:http';
import { createRequire } from 'node:module';
import { createClient, createGraphqlSseTransport, createGraphqlHttpTransport, GraphqlProtocolError, Read, Ticks } from './dist/index.js';
const require = createRequire(import.meta.url);
const root = process.env.POOLSTER_GRAPHQL_SSE_ROOT;
assert.equal(require(root+'/graphql-sse/package.json').version,'2.6.0');
assert.equal(require(root+'/graphql/package.json').version,'16.14.2');
const {buildSchema,graphql} = require(root+'/graphql');
const {createHandler} = require(root+'/graphql-sse/lib/use/http');
const schema=buildSchema(process.env.POOLSTER_SCHEMA);
const date=schema.getType('DateTime');date.parseValue=value=>new Date(value);date.serialize=value=>value.toISOString();
let closed=0;
schema.getSubscriptionType().getFields().ticks.subscribe=async function*(_source,args){
  try {yield {ticks:{when:args.start,maybe:null,list:[args.start,null]}};yield {ticks:{when:args.start,maybe:'bad',list:null}};}finally{closed++;}
};
const sse=createHandler({schema});
const server=http.createServer(async(req,res)=>{
  assert.equal(req.headers.authorization,'Bearer secret');assert.equal(req.headers['x-request'],'override');
  if(req.headers.accept==='text/event-stream'){await sse(req,res);return;}
  let body='';for await(const chunk of req)body+=chunk;const payload=JSON.parse(body);
  const result=await graphql({schema,source:payload.query,operationName:payload.operationName,variableValues:payload.variables,rootValue:{stamp:({input})=>{
    assert(input.when instanceof Date);assert(input.next.when instanceof Date);assert.equal(input.list[0],null);
    return {when:input.when,maybe:null,list:input.list};
  }}});res.setHeader('content-type','application/json');res.end(JSON.stringify(result));
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const endpoint='http://127.0.0.1:'+server.address().port;
const value=new Date('2025-01-01T00:00:00.000Z');
const codecs={DateTime:{encode:date=>date.toISOString(),decode:value=>new Date(value)}};
const headers={authorization:'Bearer secret','x-request':'initial'};
const options={headers:{'x-request':'override'}};
try {
  const transport=createGraphqlHttpTransport(endpoint,{headers,scalarCodecs:codecs});
  const subscriptionTransport=createGraphqlSseTransport(endpoint,{headers});
  const client=createClient({transport,subscriptionTransport,scalarCodecs:codecs});
  const variables={input:{when:value,list:[null,value],next:{when:value}},include:false};
  const result=await client.read(variables,options);
  assert.equal(result.kind,'success');assert(result.data.stamp.when instanceof Date);assert.equal(result.data.stamp.when.toISOString(),value.toISOString());
  assert(!Object.hasOwn(result.data.stamp,'maybe'));assert.equal(result.data.stamp.list[0],null);assert(result.data.stamp.list[1] instanceof Date);
  assert.equal(variables.input.when,value); // encode does not mutate caller objects.
  const raw=await Read(transport,{...variables,include:true},options);assert.equal(raw.data.stamp.maybe,null);
  const events=[];for await(const event of client.ticks({start:value},options))events.push(event);
  assert.equal(events.length,2);assert.equal(events[0].kind,'success');assert(events[0].data.ticks.when instanceof Date);assert.equal(events[0].data.ticks.maybe,null);
  assert.equal(events[1].kind,'partial');assert(events[1].data.ticks.when instanceof Date);assert.equal(events[1].data.ticks.maybe,null);assert(events[1].errors.length);
  const rawEvents=[];for await(const event of Ticks(createGraphqlSseTransport(endpoint,{headers,scalarCodecs:codecs}),{start:value},options))rawEvents.push(event);
  assert(rawEvents[0].data.ticks.when instanceof Date);assert(closed>=2);
  const encoder=new TextEncoder();
  const mock=(text,contentType='text/event-stream')=>async()=>new Response(new ReadableStream({start(controller){for(const byte of encoder.encode(text))controller.enqueue(Uint8Array.of(byte));controller.close();}}),{headers:{'content-type':contentType}});
  const collect=async transport=>{const events=[];for await(const event of transport.subscribe('subscription T { tick }','T',{}))events.push(event);return events;};
  const fragmented=await collect(createGraphqlSseTransport('http://unused',{fetch:mock(': heartbeat\r\nevent: next\r\ndata: {"data":\r\ndata: {"tick":"é"}}\r\n\r\nevent: complete\r\ndata: \r\n\r\n')}));assert.equal(fragmented[0].data.tick,'é');
  for(const text of ['event: next\ndata: invalid\n\n','event: next\ndata: {}\n\n','event: next\ndata: {"data":{"tick":1}}\n\n','event: unknown\ndata: {}\n\n'])await assert.rejects(collect(createGraphqlSseTransport('http://unused',{fetch:mock(text)})),GraphqlProtocolError);
  await assert.rejects(collect(createGraphqlSseTransport('http://unused',{fetch:mock('event: next\ndata: {"data":{"tick":"xxxxxxxxxxx"}}\n\n'),maxEventBytes:10})),GraphqlProtocolError);
  await assert.rejects(collect(createGraphqlSseTransport('http://unused',{fetch:mock('{}','application/json')})),GraphqlProtocolError);
  let cancelled=false,aborted=false;
  const cleanup=createGraphqlSseTransport('http://unused',{fetch:async(_url,init)=>{init.signal.addEventListener('abort',()=>{aborted=true});return new Response(new ReadableStream({start(controller){controller.enqueue(encoder.encode('event: next\ndata: {"data":{"tick":1}}\n\n'));},cancel(){cancelled=true;}}),{headers:{'content-type':'text/event-stream'}})}});
  for await(const event of cleanup.subscribe('subscription T {tick}','T',{}))break;
  assert(cancelled&&aborted);
} finally {await new Promise(resolve=>server.close(resolve));}
