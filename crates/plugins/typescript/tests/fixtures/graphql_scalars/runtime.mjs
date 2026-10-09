import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import assert from 'node:assert/strict';
import {Stamp,stamp,createClient,createGraphqlHttpTransport} from './dist/index.js';
const require=createRequire(import.meta.url);
const {buildSchema,graphql}=require(process.env.POOLSTER_GRAPHQL_JS);
const schema=buildSchema(process.env.POOLSTER_SCALAR_SCHEMA);
const scalar=schema.getType('DateTime');
scalar.parseValue=value=>{assert.equal(typeof value,'string');const date=Date.parse(value);assert.ok(Number.isFinite(date));return date;};
scalar.serialize=value=>Number(value);
let seen;
const root={stamp:args=>{seen=args;return {at:args.at,maybe:args.maybe??null,history:args.input.history,payload:args.input.payload??null,opaque:'wire-opaque'};}};
const server=createServer(async(req,res)=>{let body='';for await(const chunk of req)body+=chunk;const input=JSON.parse(body);const result=await graphql({schema,source:input.query,variableValues:input.variables,operationName:input.operationName,rootValue:root});res.setHeader('content-type','application/json');res.end(JSON.stringify(result));});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
try {
 const transport=createGraphqlHttpTransport(`http://127.0.0.1:${server.address().port}`);
 const input={at:'2026-10-01T00:00:00Z',include:false,input:{at:'2026-10-02T00:00:00Z',history:['2026-10-03T00:00:00Z'],payload:{marker:'ok'}}};
 const result=await Stamp(transport,input);
 assert.equal(result.kind,'success');
 assert.equal(result.data.stamp.at,Date.parse(input.at));
 assert.equal(typeof result.data.stamp.at,'number');
 assert.deepEqual(result.data.stamp.history,[Date.parse(input.input.history[0])]);
 assert.deepEqual(result.data.stamp.payload,{marker:'ok'});
 assert.equal(result.data.stamp.opaque,'wire-opaque');
 assert.equal('maybe' in result.data.stamp,false);
 assert.equal('maybe' in seen,false);
 assert.equal(typeof input.at,'string'); // mapping installs no client-side coercion
 const nullable=await Stamp(transport,{...input,include:true,maybe:null,input:{...input.input,maybe:null}});
 assert.equal(nullable.kind,'success');assert.equal(nullable.data.stamp.maybe,null);
 assert.equal(seen.maybe,null);
 const client=createClient({transport});
 const bound=await client.mutation.stamp(input);
 assert.equal(bound.kind,'success');assert.equal(bound.data.stamp.at,Date.parse(input.at));
 const alias=await stamp(transport,input);
 assert.equal(alias.kind,'success');assert.equal(alias.data.stamp.at,bound.data.stamp.at);
} finally {await new Promise(resolve=>server.close(resolve));}
