//! Opt-in execution checks against actual generated transport modules.
#[test]
#[ignore = "requires Node, KAJI_TSC_JS and KAJI_AXIOS_NODE_MODULES"]
fn generated_middleware_fetch_and_axios_execute() {
    let compiler = std::env::var("KAJI_TSC_JS").unwrap();
    let modules = std::env::var("KAJI_AXIOS_NODE_MODULES").unwrap();
    let directory = std::env::temp_dir().join(format!("kaji-middleware-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&modules, directory.join("node_modules")).unwrap();
    for (name, transport) in [
        ("fetch", crate::sdk::SdkTransport::Fetch),
        ("axios", crate::sdk::SdkTransport::Axios),
    ] {
        std::fs::write(
            directory.join(format!("{name}.ts")),
            crate::sdk::kaji_runtime(transport, None),
        )
        .unwrap();
    }
    std::fs::write(directory.join("consumer.ts"), r#"
import { createClient, type ClientMiddleware, type MiddlewareResponse } from './fetch'
import { createClient as axiosClient, type ClientMiddleware as AxiosMiddleware } from './axios'
const replace: ClientMiddleware = async (request, next) => {
  const result = await next({ ...request, body: { rewritten: true }, query: { page: 2 } }) as MiddlewareResponse
  return { ...result, data: { transformed: result.data } }
}
const recovery: AxiosMiddleware = async (request, next) => {
  try { return await next(request) } catch (cause) { throw new Error('customer error', { cause }) }
}
createClient({ middleware: [replace] })
axiosClient({ middleware: [recovery] })
"#).unwrap();
    let result = std::process::Command::new("node")
        .arg(&compiler)
        .args([
            "--strict",
            "--target",
            "ES2022",
            "--module",
            "commonjs",
            "--moduleResolution",
            "node",
            "--lib",
            "ES2022,DOM,DOM.Iterable",
            "--skipLibCheck",
            "--outDir",
            "compiled",
            "fetch.ts",
            "axios.ts",
            "consumer.ts",
        ])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::write(directory.join("run.cjs"), r#"
const assert = require('node:assert/strict')
const axios = require('axios').default
const runtimes = [require('./compiled/fetch.js'), require('./compiled/axios.js')]
;(async () => {
for (const [index, runtime] of runtimes.entries()) {
  const trace = []; let attempts = 0; let observed; let hookCalls = 0
  const send = async request => { observed = request; attempts++; return attempts === 1 ? {status:503,data:{},headers:{'content-type':'application/json'}} : {status:200,data:{original:true},headers:{'content-type':'application/json'}} }
  const config = index === 0 ? {baseUrl:'https://example.test', fetch: async (url, init) => { const response = await send({url,...init}); return new Response(JSON.stringify(response.data), {status:response.status,headers:response.headers}) }} : { client: axios.create({adapter: async request => ({...(await send(request)),config:request,statusText:'ok'})}) }
  const client = runtime.createClient({...config,retry:{maxAttempts:2,initialDelayMs:0,maxDelayMs:0},hooks:{beforeRequest:()=>hookCalls++},middleware:[
    async (request,next) => { trace.push('outer-before'); request.query.page=2; const result=await next({...request,url:'/rewritten',body:{changed:true},headers:{'x-customer':'yes'}}); trace.push('outer-after'); return {...result,data:{changed:result.data}} },
    async (request,next) => {trace.push('inner-before');const result=await next(request);trace.push('inner-after');return result}
  ]})
  const original={method:'PUT',url:'/old',query:{page:1},headers:{'idempotency-key':'safe'},contentType:{request:'application/json'}}
  // Retrying PUT is allowed by the built-in idempotent method policy.
  const result=await client(original)
  assert.equal(attempts,2);assert.equal(hookCalls,1);assert.deepEqual(trace,['outer-before','inner-before','inner-after','outer-after'])
  assert.equal(original.query.page,1);assert.deepEqual(result.data,{changed:{original:true}})
  assert.match(observed.url,/rewritten/);assert.equal(observed.headers.get('x-customer'),'yes')
  assert.equal(index===0 ? observed.body : observed.data,'{"changed":true}')
  assert.equal(index===0 ? new URL(observed.url).searchParams.get('page') : observed.params.page,index===0 ? '2' : 2)
  let sent=false
  const cached=runtime.createClient({...config,middleware:[async()=>({status:200,contentType:'application/json',data:'cached',headers:{}}),async()=>{sent=true}]})
  assert.equal((await cached({method:'GET',url:'/cached'})).data,'cached');assert.equal(sent,false);assert.equal(attempts,2)
  // Separate failing transport ensures both HTTP error paths can be recovered.
  const failConfig=index===0?{fetch:async()=>new Response('{}',{status:401})}:{client:axios.create({adapter:async config=>({status:401,data:{},headers:{},config,statusText:'denied'})})}
  const failing=runtime.createClient({...failConfig,retry:false,middleware:[async(req,next)=>{try{return await next(req)}catch(error){throw new Error('rewritten',{cause:error})}}]})
  await assert.rejects(()=>failing({method:'GET',url:'https://example.test/fail'}),error=>error.message==='rewritten'&&error.cause.status===401)
  const recovery=runtime.createClient({...failConfig,retry:false,middleware:[async(req,next)=>{try{return await next(req)}catch{return {status:200,data:'recovered',headers:{},contentType:'application/json'}}}]})
  assert.equal((await recovery({method:'GET',url:'https://example.test/fail'})).data,'recovered')
  const list=[async()=>({status:200,data:'snapshot'})];const snapshot=runtime.createClient({middleware:list});list.length=0;assert.equal((await snapshot({method:'GET',url:'/snapshot'})).data,'snapshot')
  if(index===0) {
    for(const headers of [new Headers({'x-original':'yes'}),[['x-original','yes']]]) {
      const originalHeaders=headers
      const headerClient=runtime.createClient({middleware:[async(request)=>{const copied=new Headers(request.headers);assert.equal(copied.get('x-original'),'yes');assert.notEqual(request.headers,originalHeaders);return {status:200,data:null}}]})
      await headerClient({method:'GET',url:'/headers',headers})
    }
  }
  const duplicate=runtime.createClient({middleware:[async(req,next)=>{await next(req);return next(req)},async()=>({status:200,data:null})]})
  await assert.rejects(()=>duplicate({method:'GET',url:'/unused'}),/only be called once/)
  const shape={kind:'object',required:['id','rows','secret'],fields:{id:{kind:'integer'},rows:{kind:'array',items:{ref:'Row'}},secret:{kind:'string',writeOnly:true}}}
  const refs={Row:{kind:'object',required:['name'],fields:{name:{kind:'string',literals:['known']},child:{ref:'Row',nullable:true}}}}
  const plan={refs,requests:{},responses:{'2XX':{'application/json':shape}}}
  const request={method:'GET',url:'/shape',jsonPlan:plan}
  const valid={id:3,rows:[{name:'future-enum',child:null}],extra:'retained'}
  const strict=runtime.createClient({validateResponses:true,middleware:[async()=>({status:201,contentType:'application/json; charset=utf-8',data:valid})]})
  assert.deepEqual((await strict(request)).data,valid)
  for(const bad of [{id:'credential-value',rows:[]},{id:1,rows:[{name:42}]},{id:1,rows:null},{rows:[]},{id:1.5,rows:[]}]) {
    let terminalCalls=0
    const invalid=runtime.createClient({validateResponses:true,middleware:[async()=>({status:200,contentType:'application/json',data:bad}),async()=>{terminalCalls++;throw new Error('must not run')}]})
    await assert.rejects(()=>invalid(request),error=>error instanceof runtime.ResponseDecodeError && error.path.startsWith('$') && !error.message.includes('credential-value'))
    assert.equal(terminalCalls,0)
    assert.deepEqual((await invalid({...request,validateResponses:false})).data,bad)
  }
  let strictAttempts=0
  const invalidWire=JSON.stringify({id:'bad',rows:[]})
  const strictTransport=index===0?{fetch:async()=>{strictAttempts++;return new Response(invalidWire,{headers:{'content-type':'application/json'}})}}:{client:axios.create({adapter:async config=>{strictAttempts++;return {status:200,data:invalidWire,headers:{'content-type':'application/json'},config,statusText:'ok'}}})}
  await assert.rejects(()=>runtime.createClient({...strictTransport,validateResponses:true,retry:{maxAttempts:3}})({...request,url:'https://example.test/shape'}),runtime.ResponseDecodeError)
  assert.equal(strictAttempts,1)
  const decoded=runtime.createClient({...strictTransport,validateResponses:true,codecs:{'application/json':{decode:()=>valid}}})
  assert.deepEqual((await decoded({...request,url:'https://example.test/shape'})).data,valid)
  const statuses={refs:{},requests:{},responses:{'200':{'application/json':{kind:'string'}},'2XX':{'application/json':{kind:'integer'}},default:{'application/json':{kind:'boolean'}}}}
  const envelope=(status,data)=>runtime.createClient({validateResponses:true,middleware:[async()=>({status,contentType:'application/json',data})]})({...request,jsonPlan:statuses})
  await envelope(200,'exact');await envelope(201,2)
  await assert.rejects(()=>envelope(200,2),runtime.ResponseDecodeError)
  await envelope(204,undefined)
  await runtime.createClient({validateResponses:true,middleware:[async()=>({status:200,contentType:'application/json',data:undefined})]})({...request,method:'HEAD'})
  await runtime.createClient({validateResponses:true,middleware:[async()=>({status:202,contentType:'application/json',data:true})]})({...request,jsonPlan:{...statuses,responses:{default:statuses.responses.default}}})
  const rewrite=runtime.createClient({validateResponses:true,middleware:[async(req,next)=>({...await next(req),data:{id:false,rows:[]}}),async()=>({status:200,contentType:'application/json',data:valid})]})
  await assert.rejects(()=>rewrite(request),runtime.ResponseDecodeError)
  const unconstrained=runtime.createClient({validateResponses:true,middleware:[async()=>({status:200,contentType:'application/json',data:'anything'})]})
  assert.equal((await unconstrained({...request,jsonPlan:undefined})).data,'anything')
  runtime.assertResponseShape({a:'ok',b:true},{kind:'allOf',variants:[{kind:'object',required:['a'],fields:{a:{kind:'string'}}},{kind:'object',required:['b'],fields:{b:{kind:'boolean'}}}]})
  assert.throws(()=>runtime.assertResponseShape({a:'ok',b:4},{kind:'allOf',variants:[{kind:'object',fields:{a:{kind:'string'}}},{kind:'object',fields:{b:{kind:'boolean'}}}]}),runtime.ResponseDecodeError)
  runtime.assertResponseShape(null,{kind:'union',variants:[{kind:'string'},{kind:'null'}]})
  runtime.assertResponseShape(null,{ref:'Nullable'},{Nullable:{kind:'string',nullable:true}})
  runtime.assertResponseShape('ok',{kind:'union',variants:[{kind:'string'},{kind:'integer'}]})
  assert.throws(()=>runtime.assertResponseShape(false,{kind:'union',variants:[{kind:'string'},{kind:'integer'}]}),runtime.ResponseDecodeError)
  const stream=new ReadableStream({start(controller){controller.enqueue(new TextEncoder().encode('data: {"ok":true}\n\n'));controller.close()}})
  const streamClient=runtime.createClient({middleware:[async()=>index===0?new Response(stream):{status:200,data:stream,headers:{},contentType:'text/event-stream'}]})
  const events=await runtime.toEventStream(streamClient({method:'GET',url:'/events',responseType:'stream'})); const values=[];for await(const event of events)values.push(event);assert.deepEqual(values,[{ok:true}])
}
})().catch(error=>{console.error(error);process.exitCode=1})
"#).unwrap();
    let result = std::process::Command::new("node")
        .arg("run.cjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Node, KAJI_TSC_JS and KAJI_AXIOS_NODE_MODULES"]
fn bundled_middleware_is_enabled_without_customer_registration() {
    use kaji_core::{
        Api, HttpMethod, Operation, OperationMediaType, OperationResponse, SchemaKind, SchemaValue,
        customization::BundledMiddleware,
    };
    let root = tempfile::tempdir().unwrap();
    let compiler = std::env::var("KAJI_TSC_JS").unwrap();
    let modules = std::env::var("KAJI_AXIOS_NODE_MODULES").unwrap();
    let api = Api {
        name: "Contacts".into(),
        version: "1.0.0".into(),
        operations: vec![Operation {
            id: "listContacts".into(),
            method: HttpMethod::Get,
            path: "/contacts".into(),
            responses: vec![OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::new(SchemaKind::String)),
                }],
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    #[cfg(unix)]
    std::os::unix::fs::symlink(modules, root.path().join("node_modules")).unwrap();
    let mut generated = kaji_core::GeneratedTree::default();
    for (name, sdk) in [
        ("fetch", crate::sdk().fetch()),
        ("axios", crate::sdk().axios()),
    ] {
        let package = crate::package(name).with(sdk.client_name("BundledSdk").flat()).middleware(BundledMiddleware {
            path: "middleware/policy.ts".into(), symbol: "authorPolicy".into(), async_symbol: None,
            contents: r#"import type { ClientMiddleware, MiddlewareResponse } from '../.kaji/client'
export const authorPolicy: ClientMiddleware = async (request, next) => {
  const response = await next({ ...request, query: { ...request.query, tenant: 'author' } }) as MiddlewareResponse
  return { ...response, data: 'bundled:' + response.data }
}
"#.into(),
        });
        let tree = kaji_core::engine::Packages::new()
            .package(package)
            .generate(&api, None)
            .unwrap();
        assert!(
            tree.get(format!("{name}/README.md"))
                .unwrap()
                .contains("No constructor registration")
        );
        generated.append(tree).unwrap();
    }
    generated.write_to(root.path()).unwrap();
    std::fs::write(root.path().join("tsconfig.json"), r#"{"compilerOptions":{"strict":true,"target":"ES2022","module":"commonjs","moduleResolution":"node","lib":["ES2022","DOM","DOM.Iterable"],"skipLibCheck":true,"outDir":"compiled"},"include":["fetch/**/*.ts","axios/**/*.ts"]}"#).unwrap();
    // Hidden runtime modules are followed through SDK/source imports by tsc.
    let result = std::process::Command::new("node")
        .arg(compiler)
        .arg("--project")
        .arg("tsconfig.json")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::write(root.path().join("run.cjs"), r#"
const assert=require('node:assert/strict')
;(async()=>{
  const FetchSdk=require('./compiled/fetch/index').BundledSdk
  const fetchSdk=new FetchSdk({baseUrl:'https://unused.example',fetch:async(url)=>{
    assert.equal(new URL(url).searchParams.get('tenant'),'author')
    return new Response(JSON.stringify('native'),{status:200,headers:{'content-type':'application/json'}})
  }})
  assert.equal(await fetchSdk.listContacts(),'bundled:native')
  const AxiosSdk=require('./compiled/axios/index').BundledSdk
  const axios=require('axios')
  const driver=axios.create({adapter:async(config)=>{
    assert.equal(config.params.tenant,'author')
    return {data:'native',status:200,statusText:'OK',headers:{'content-type':'application/json'},config}
  }})
  const axiosSdk=new AxiosSdk({baseUrl:'https://unused.example',client:driver})
  assert.equal(await axiosSdk.listContacts(),'bundled:native')
})().catch(error=>{console.error(error);process.exitCode=1})
"#).unwrap();
    let result = std::process::Command::new("node")
        .arg("run.cjs")
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
