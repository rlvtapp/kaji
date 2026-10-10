const assert=require('node:assert/strict');
const http=require('node:http');
const {pathToFileURL}=require('node:url');
const root=process.env.POOLSTER_GRAPHQL_JS_ROOT;
assert.equal(require(root+'/graphql/package.json').version,'16.14.2');
const {buildSchema,graphql}=require(root+'/graphql');
const schema=buildSchema('directive @defer(if:Boolean! = true,label:String) on FRAGMENT_SPREAD | INLINE_FRAGMENT\ndirective @stream(if:Boolean! = true,label:String,initialCount:Int! = 0) on FIELD\ntype Query { users:[User!]! } type User { id:ID! name:String nickname:String }');
let mode='normal';let cancelled=false;
const server=http.createServer(async(req,res)=>{
 let body='';for await(const chunk of req)body+=chunk;
 const request=JSON.parse(body);if(request.variables.tag!==undefined)assert.equal(request.variables.tag,'UPPER');assert.equal(request.operationName,'Users');assert.ok(req.headers.accept.includes('deferSpec=20220824'));
 const result=await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:{users:[{id:'1',name:'Zoë',nickname:null},{id:'2',name:'Ada',nickname:null}]}});
 assert.equal(result.errors,undefined);
 if(mode==='json'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));return;}
 res.setHeader('Content-Type',`multipart/mixed;boundary="random_boundary_17";deferSpec=${mode==='version'?'20250901':'20220824'}`);
 const payloads=[{data:{users:[{id:'1'}]},hasNext:true},{incremental:[{path:['users',0],label:'extra',data:{name:'Zoë',nickname:null}}],hasNext:true},{incremental:[{path:['users',1],label:'list',items:[{id:'2',name:'Ada',nickname:null}]}],hasNext:false}];
 if(mode==='error'){payloads[1].incremental[0].errors=[{message:'nickname failed',path:['users',0,'nickname']}];}
 if(mode==='truncated'){payloads.pop();}
 if(mode==='newer'){payloads[1]={pending:[{id:'1',path:['users',0]}],hasNext:true};}
 if(mode==='cancel'){req.on('close',()=>{cancelled=true;});}
 for(const payload of payloads){
  if(res.destroyed)break;
  const bytes=Buffer.from('--random_boundary_17\r\nContent-Type: application/json\r\n\r\n'+JSON.stringify(payload)+'\r\n');
  for(let i=0;i<bytes.length;i+=7){if(res.destroyed)break;res.write(bytes.subarray(i,i+7)); await new Promise(r=>setImmediate(r));}
  if(mode==='cancel') await new Promise(r=>setTimeout(r,100));
 }
 if(!res.destroyed)res.end('--random_boundary_17--\r\n');
});
(async()=>{
 const sdk=await import(pathToFileURL(process.argv[2]));
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const options={endpoint:`http://127.0.0.1:${server.address().port}`};
 try{
  const snapshots=[];for await(const value of sdk.users(options,{}))snapshots.push(value);
  assert.equal(snapshots.length,3);assert.deepEqual(snapshots[0].data,{users:[{id:'1'}]});assert.equal(snapshots[0].complete,false);
  assert.equal(snapshots[1].data.users[0].name,'Zoë');assert.equal(snapshots[1].patches[0].label,'extra');
  assert.deepEqual(snapshots[2].data,{users:[{id:'1',name:'Zoë',nickname:null},{id:'2',name:'Ada',nickname:null}]});assert.equal(snapshots[2].complete,true);
  mode='normal';const codecFrames=[];for await(const value of sdk.executeIncremental({...options,scalarCodecs:{String:{encode:v=>v.toUpperCase(),decode:v=>'decoded:'+v}}},'Users','query Users {users @stream(initialCount:1) {id ... @defer {name nickname}}}',{tag:'upper'},{fields:[['tag',{scalar:'String'}]]},{fields:[['users',{list:{fields:[['id',{scalar:'ID'}],['name',{scalar:'String'}],['nickname',{scalar:'String'}]]}}]]},{}))codecFrames.push(value);assert.equal(codecFrames[1].data.users[0].name,'decoded:Zoë');assert.equal(codecFrames[2].data.users[0].name,'decoded:Zoë');assert.equal(codecFrames[2].data.users[1].name,'decoded:Ada');
  mode='error';const errors=[];for await(const value of sdk.users(options,{}))errors.push(value);assert.equal(errors[2].errors[0].message,'nickname failed');
  mode='json';const fallback=[];for await(const value of sdk.users(options,{}))fallback.push(value);assert.equal(fallback.length,1);assert.equal(fallback[0].complete,true);
  for(mode of ['version','newer','truncated'])await assert.rejects(async()=>{for await(const _ of sdk.users(options,{})){};});
  const initial=sdk.applyIncrementalPayload(undefined,{data:{users:[{id:'1'}]},hasNext:true});
  assert.throws(()=>sdk.applyIncrementalPayload(initial,{incremental:[{path:['users',5],items:[{}]}],hasNext:false}));
  assert.throws(()=>sdk.applyIncrementalPayload(initial,{incremental:[{path:['__proto__'],data:{polluted:true}}],hasNext:false}));
  assert.equal({}.polluted,undefined);
  const nulled=sdk.applyIncrementalPayload(initial,{incremental:[{path:['users',0],data:null,errors:[{message:'failed'}]}],hasNext:false});assert.equal(nulled.data.users[0],null);
  mode='cancel';const controller=new AbortController();await assert.rejects(async()=>{for await(const value of sdk.users({...options,signal:controller.signal},{})){controller.abort();}},error=>error.name==='AbortError');
  await new Promise(r=>setTimeout(r,150));
  console.log('incremental passed: defer/stream merge, snapshots, Unicode chunks, null/errors, JSON fallback, version/malformed/truncation, abort');
 }finally{server.closeAllConnections();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
