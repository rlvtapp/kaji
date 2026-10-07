//! Execute actual generated operation functions and both native transports.
#[test]
#[ignore = "requires Node, KAJI_TSC_JS and KAJI_AXIOS_NODE_MODULES"]
fn generated_idempotency_fetch_and_axios_execute() {
    use kaji_core::{
        Api, HttpMethod, Operation, OperationParameter, OperationResponse, SchemaKind, SchemaValue,
    };
    let compiler = std::env::var("KAJI_TSC_JS").unwrap();
    let modules = std::env::var("KAJI_AXIOS_NODE_MODULES").unwrap();
    let mut write = Operation {
        id: "write".into(),
        path: "/write".into(),
        method: HttpMethod::Post,
        parameters: vec![OperationParameter {
            name: "X-Key".into(),
            location: "header".into(),
            required: false,
            schema: Some(SchemaValue::new(SchemaKind::String)),
            description: None,
            annotations: Default::default(),
        }],
        responses: vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::String),
        )],
        ..Default::default()
    };
    write.annotations.insert(
        "x-kaji-idempotency-resolved".into(),
        serde_json::json!({"header":"X-Key", "parameter_name":"X-Key", "auto_generate":true}),
    );
    let mut patch = write.clone();
    patch.id = "patch".into();
    patch.method = HttpMethod::Patch;
    let mut manual = write.clone();
    manual.id = "manual".into();
    manual
        .annotations
        .get_mut("x-kaji-idempotency-resolved")
        .unwrap()["auto_generate"] = serde_json::json!(false);
    let mut unsafe_write = write.clone();
    unsafe_write.id = "unsafeWrite".into();
    unsafe_write.annotations.clear();
    let mut unsafe_patch = unsafe_write.clone();
    unsafe_patch.id = "unsafePatch".into();
    unsafe_patch.method = HttpMethod::Patch;
    let api = Api {
        name: "Keys".into(),
        version: "1.0.0".into(),
        operations: vec![write, patch, manual, unsafe_write, unsafe_patch],
        ..Default::default()
    };
    for (label, transport) in [
        ("fetch", crate::sdk::SdkTransport::Fetch),
        ("axios", crate::sdk::SdkTransport::Axios),
    ] {
        let temp = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&modules, temp.path().join("node_modules")).unwrap();
        let mut config = crate::sdk::SdkConfig::new("sdk");
        config.transport = transport;
        config.group_by_tag = false;
        let tree = crate::sdk::generate_sdk(&api, &config, None).unwrap();
        tree.write_to(temp.path()).unwrap();
        let mut command = std::process::Command::new("node");
        command.arg(&compiler).args([
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
        ]);
        for (path, _) in tree
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "ts"))
        {
            command.arg(path);
        }
        let output = command.current_dir(temp.path()).output().unwrap();
        assert!(
            output.status.success(),
            "{label}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(temp.path().join("run.cjs"), r#"
const assert=require('node:assert/strict');const axios=require('axios').default;
const runtime=require('./compiled/.kaji/client.js');
const {write}=require('./compiled/clients/write.js');const {patch}=require('./compiled/clients/patch.js');
const {manual}=require('./compiled/clients/manual.js');const {unsafeWrite}=require('./compiled/clients/unsafeWrite.js');const {unsafePatch}=require('./compiled/clients/unsafePatch.js');
;(async()=>{
 const keys=[];let calls=0;
 const send=request=>{calls++;keys.push(request.headers instanceof Headers?request.headers.get('X-Key'):request.headers.get('X-Key'));return {status:calls%2===1?503:200,data:'ok',headers:{'content-type':'application/json'}}};
 const driver=process.argv[2]==='fetch'?{fetch:async(url,init)=>{const result=send(init);return new Response(JSON.stringify(result.data),{status:result.status,headers:result.headers})}}:{client:axios.create({adapter:async request=>({...send(request),config:request,statusText:'ok'})})};
 const client=runtime.createClient({...driver,baseUrl:'https://example.test',retry:{maxAttempts:2,initialDelayMs:0,maxDelayMs:0}});
 await write({client});assert.equal(keys.length,2);assert.equal(keys[0],keys[1]);assert.match(keys[0],/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
 await write({client});assert.notEqual(keys[0],keys[2]);
 const original={headers:{'x-key':'caller'}};await write({...original,client});assert.deepEqual(keys.slice(4,6),['caller','caller']);assert.deepEqual(original,{headers:{'x-key':'caller'}});
 await patch({client});assert.equal(keys[6],keys[7]);
 let before=keys.length;await assert.rejects(()=>unsafeWrite({client,headers:{'X-Key':'caller'}}));assert.equal(keys.length,before+1);
 calls=0;before=keys.length;await assert.rejects(()=>unsafePatch({client}));assert.equal(keys.length,before+1);
 calls=0;before=keys.length;await assert.rejects(()=>write({client,headers:{'x-key':''}}));assert.equal(keys.length,before+1);assert.equal(keys.at(-1),'');
 calls=0;before=keys.length;const blank={headers:{'x-key':'   '}};await assert.rejects(()=>write({...blank,client}));assert.equal(keys.length,before+1);assert.deepEqual(blank,{headers:{'x-key':'   '}});
 let blankObserved;await write({...blank,client:async request=>{blankObserved=request.headers['x-key'];return {status:200,data:'ok',headers:{},contentType:'application/json'}}});assert.equal(blankObserved,'   ');

 calls=0;before=keys.length;await assert.rejects(()=>manual({client}));assert.equal(keys.length,before+1);assert.equal(keys.at(-1)==null,true);
 calls=0;before=keys.length;await manual({client,headers:{'X-Key':'manual'}});assert.deepEqual(keys.slice(before),['manual','manual']);
 let observed;await write({client:async request=>{observed=request;return {status:200,data:'ok',headers:{},contentType:'application/json'}}});assert.match(observed.headers['X-Key'],/^[0-9a-f-]{36}$/);assert.equal(observed.idempotencyHeader,'X-Key');
 const originalTimeout=globalThis.setTimeout;const delays=[];
 globalThis.setTimeout=(callback,delay)=>{delays.push(delay);return originalTimeout(callback,0)};
 for(const [headers,expected] of [
  [{'retry-after-ms':'25','retry-after':'99'},25],
  [{'retry-after-ms':'99999999999'},50],
  [{'retry-after-ms':'1.5'},1.5],
  [{'retry-after-ms':'NaN','retry-after':'0'},0],
  [{'retry-after-ms':'-1','retry-after':'-1'},7],
  [{'retry-after':new Date(Date.now()+60000).toUTCString()},50],
  [{'retry-after-ms':'1e3'},7]
 ]) {
  let attempts=0;const respond=()=>({status:++attempts===1?503:200,data:'ok',headers:{'content-type':'application/json',...headers}});
  const config=process.argv[2]==='fetch'?{fetch:async()=>{const result=respond();return new Response(JSON.stringify(result.data),{status:result.status,headers:result.headers})}}:{client:axios.create({adapter:async request=>({...respond(),config:request,statusText:'ok'})})};
  const retryClient=runtime.createClient({...config,baseUrl:'https://example.test',retry:{maxAttempts:2,initialDelayMs:7,maxDelayMs:50}});
  await retryClient({method:'GET',url:'/delays'});assert.equal(delays.at(-1),expected);assert.equal(attempts,2);
 }
 globalThis.setTimeout=originalTimeout;
 const crypto=globalThis.crypto;
 Object.defineProperty(globalThis,'crypto',{value:{getRandomValues:crypto.getRandomValues.bind(crypto)},configurable:true});
 await write({client:async request=>{assert.match(request.headers['X-Key'],/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);return {status:200,data:'ok',headers:{},contentType:'application/json'}}});
 Object.defineProperty(globalThis,'crypto',{value:undefined,configurable:true});
 assert.throws(()=>write({client}),/secure randomness/);
 Object.defineProperty(globalThis,'crypto',{value:crypto,configurable:true});
})().catch(error=>{console.error(error);process.exitCode=1});
"#).unwrap();
        let output = std::process::Command::new("node")
            .args(["run.cjs", label])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{label}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
