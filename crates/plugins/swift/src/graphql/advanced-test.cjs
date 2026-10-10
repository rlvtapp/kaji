const assert=require('node:assert/strict');const http=require('node:http');const {spawn}=require('node:child_process');
const root=process.env.POOLSTER_GRAPHQL_JS_ROOT;assert.equal(require(root+'/graphql/package.json').version,'16.14.2');const {buildSchema,graphql,parse,subscribe}=require(root+'/graphql');
const schema=buildSchema('scalar Date directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT type Query { readUser(id:ID!):User } type Subscription { readUser(id:ID!):User } type User { name:Date! nickname:String }');
let mode='normal',closed=false;
const server=http.createServer(async(req,res)=>{try{let body='';for await(const chunk of req)body+=chunk;const request=JSON.parse(body);assert.equal(request.variables.id,'1!');assert.equal(request.operationName,'ReadUser');
res.on('close',()=>{closed=true;});
if(process.argv[4]==='sse'){
if(mode==='unknown'){res.setHeader('Content-Type','text/event-stream');res.end('event: error\r\ndata: {"message":"bad"}\r\n\r\n');return;}
if(mode==='truncated'){res.setHeader('Content-Type','text/event-stream');res.end('event: next\r\ndata: {"data":{"readUser":{"name":"Ada","nickname":null}}}\r\n\r\n');return;}
 assert.equal(req.headers.accept,'text/event-stream');res.setHeader('Content-Type','text/event-stream');
 const events=await subscribe({schema,document:parse(request.query),operationName:request.operationName,variableValues:request.variables,rootValue:{async *readUser(){yield {readUser:{name:'Ada',nickname:null}};yield {readUser:{name:'Ada',nickname(){throw new Error('nickname failed');}}};}}});
 for await(const event of events){if(res.destroyed)break;const bytes=Buffer.from(': heartbeat\r\nevent: next\r\ndata: '+JSON.stringify(event)+'\r\n\r\n');for(let i=0;i<bytes.length;i+=11){res.write(bytes.subarray(i,i+11));await new Promise(r=>setImmediate(r));}if(mode==='cancel')await new Promise(r=>setTimeout(r,200));}res.end('event: complete\r\n\r\n');
}else{
 assert.ok(req.headers.accept.includes('deferSpec=20220824'));const result=await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:{readUser:{name:'Ada',nickname:null}}});assert.equal(result.errors,undefined);
 if(mode==='json'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));return;}
 if(mode==='version'){res.setHeader('Content-Type','multipart/mixed;boundary="variable_boundary";deferSpec=20250901');res.end();return;}
 if(mode==='malformed'){res.setHeader('Content-Type','multipart/mixed;boundary="variable_boundary";deferSpec=20220824');res.end('--variable_boundary\r\nContent-Type: application/json\r\n\r\n{"pending":[],"hasNext":false}\r\n--variable_boundary--\r\n');return;}
 res.setHeader('Content-Type','multipart/mixed;boundary="variable_boundary";deferSpec=20220824');
 for(const payload of [{data:{readUser:{}},hasNext:true},{incremental:[{path:['readUser'],data:result.data.readUser}],hasNext:false}]){if(res.destroyed)break;res.write('--variable_boundary\r\nContent-Type: application/json\r\n\r\n'+JSON.stringify(payload)+'\r\n');if(mode==='cancel')await new Promise(r=>setTimeout(r,200));}if(mode==='truncated')res.end();else res.end('--variable_boundary--\r\n');
}}catch(e){res.statusCode=500;res.end(e.stack);}});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));try{for(mode of (process.argv[4]==='sse'?['normal','unknown','truncated','cancel']:['normal','json','version','malformed','truncated','cancel'])){closed=false;const child=spawn(process.argv[3],[`http://127.0.0.1:${server.address().port}`,...(mode==='normal'?[]:[mode])],{cwd:process.argv[2],stdio:'inherit'});const code=await new Promise(r=>child.on('exit',r));assert.equal(code,0,mode);if(mode==='cancel'){await new Promise(r=>setTimeout(r,50));assert.equal(closed,true,'Cancellation must close server connection');}}}finally{server.closeAllConnections();await new Promise(r=>server.close(r));}})().catch(e=>{console.error(e);process.exitCode=1;});
