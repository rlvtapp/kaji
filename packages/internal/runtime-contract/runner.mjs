/** Runs generated public SDK methods against shared loopback wire scenarios. */
import assert from 'node:assert/strict'
import {readFile,mkdtemp,cp,mkdir,writeFile,readdir,rm} from 'node:fs/promises'
import {createServer} from 'node:http'
import {spawn} from 'node:child_process'
import {tmpdir} from 'node:os'
import {join,resolve,dirname,delimiter} from 'node:path'
import {fileURLToPath,pathToFileURL} from 'node:url'
const here=dirname(fileURLToPath(import.meta.url))
export const manifest=JSON.parse(await readFile(join(here,'scenarios.json'),'utf8'))
export function validateManifest(value){
    assert.equal(value.schema_version,1)
    const ids=value.scenarios.map(s=>s.id);assert.equal(new Set(ids).size,ids.length)
    for(const scenario of value.scenarios){assert.match(scenario.id,/^[a-z0-9_]+$/);assert.ok(scenario.responses.length);assert.ok(['success','error'].includes(scenario.outcome));assert.equal(scenario.requests,scenario.responses.length)}
    for(const coverage of Object.values(value.coverage)){
        const declared=[...coverage.supported,...Object.keys(coverage.unsupported??{})]
        assert.deepEqual([...declared].sort(),[...ids].sort());assert.equal(new Set(declared).size,declared.length)
        for(const reason of Object.values(coverage.unsupported??{}))assert.ok(reason.length>10)
    }
}
validateManifest(manifest)
export function validateKeyLifetime(scenario,requests){
    const keys=requests.map(request=>request.headers['x-once'])
    if(scenario.key_policy==='stable'||scenario.key_policy==='fresh'){
        for(const key of keys)assert.match(key,/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,`${scenario.id}: secure UUID`)
        assert.equal(new Set(keys).size,scenario.key_policy==='fresh'?keys.length:1,`${scenario.id}: key lifetime`)
    }
    if(scenario.key_policy==='override')for(const key of keys)assert.equal(key,scenario.caller_key,`${scenario.id}: caller key`)
    if(scenario.key_policy==='blank')for(const key of keys)assert.equal((key??'').trim(),'',`${scenario.id}: blank key replaced`)
}
export async function wireServer(value=manifest){
    const counts=new Map();const requests=new Map();const violations=[]
    const server=createServer(async(request,response)=>{
        const scenario=value.scenarios.find(s=>request.headers.authorization===`Bearer ${s.id}`)
        if(!scenario){violations.push('missing or unexpected bearer credential');response.writeHead(401);response.end('{}');return}
        const operation=scenario.operation??value.operation
        const url=new URL(request.url,'http://fixture.test')
        if(request.method!==operation.method||url.pathname!==operation.path)violations.push(`${scenario.id}: method/path mismatch`)
        if(request.headers['x-contract-middleware']!=='yes')violations.push(`${scenario.id}: consumer middleware header missing`)
        const count=counts.get(scenario.id)??0;counts.set(scenario.id,count+1)
        const expected=scenario.requests_wire?.[count]
        if(expected?.query)try{assert.deepEqual(Object.fromEntries(Object.keys(expected.query).map(key=>[key,Array.isArray(expected.query[key])?url.searchParams.getAll(key):url.searchParams.get(key)])),expected.query);assert.equal(new Set(url.searchParams.keys()).size,Object.keys(expected.query).length)}catch{violations.push(`${scenario.id}: query mismatch`)}
        if(expected?.headers)for(const [name,value]of Object.entries(expected.headers))if(request.headers[name]!==value)violations.push(`${scenario.id}: header mismatch ${name}`)
        if(expected?.body){let body='';for await(const chunk of request)body+=chunk;try{assert.deepEqual(JSON.parse(body),expected.body)}catch{violations.push(`${scenario.id}: body mismatch`)}}
        const captures=requests.get(scenario.id)??[];captures.push({method:request.method,path:request.url,headers:{...request.headers}});requests.set(scenario.id,captures)
        const reply=scenario.responses[Math.min(count,scenario.responses.length-1)]
        response.writeHead(reply.status,{'content-type':'application/json','connection':'close','retry-after':'0'})
        response.end(reply.body)
    })
    await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve)})
    return {url:`http://127.0.0.1:${server.address().port}`,counts,requests,violations,close:()=>new Promise(resolve=>server.close(resolve))}
}
export function execute(program,args,options={}){
    return new Promise((resolve,reject)=>{
        const child=spawn(program,args,{cwd:options.cwd,env:{...process.env,...options.env},stdio:['ignore','pipe','pipe']})
        let stdout='',stderr='';child.stdout.on('data',data=>stdout+=data);child.stderr.on('data',data=>stderr+=data)
        const timer=setTimeout(()=>{child.kill('SIGKILL');reject(new Error(`${program} timed out`))},options.timeout??120000)
        child.on('error',error=>{clearTimeout(timer);reject(error)})
        child.on('close',code=>{clearTimeout(timer);code===0?resolve(stdout):reject(new Error(`${program} exited ${code}\n${stdout}\n${stderr}`))})
    })
}
async function files(directory,extension){const result=[];for(const entry of await readdir(directory,{withFileTypes:true})){const path=join(directory,entry.name);if(entry.isDirectory())result.push(...await files(path,extension));else if(entry.name.endsWith(extension))result.push(path)}return result}
export async function prepare(language,sdk,root,exec=execute){
    const harness=join(here,'harnesses')
    switch(language){
        case 'python':return ['python3',[join(harness,'python.py')],{PYTHONPATH:join(sdk,'src')}]
        case 'ruby':return ['ruby',['-I',join(sdk,'lib'),join(harness,'ruby.rb')],{}]
        case 'go':{
            await mkdir(join(sdk,'cmd','runtime_contract'),{recursive:true});await cp(join(harness,'go.go'),join(sdk,'cmd','runtime_contract','main.go'))
            const binary=join(root,'go-probe');await exec('go',['build','-o',binary,'./cmd/runtime_contract'],{cwd:sdk,env:{GOCACHE:process.env.GOCACHE??join(root,'go-cache')}});return [binary,[],{}]
        }
        case 'rust':{
            await mkdir(join(sdk,'examples'),{recursive:true});await cp(join(harness,'rust.rs'),join(sdk,'examples','runtime_contract.rs'))
            const cargo=await readFile(join(sdk,'Cargo.toml'),'utf8');await writeFile(join(sdk,'Cargo.toml'),cargo+'\n[dev-dependencies]\ntokio = { version = "1", features = ["macros", "rt-multi-thread"] }\n')
            const target=process.env.POOLSTER_RUNTIME_RUST_TARGET??join(root,'rust-target')
            await exec('cargo',['build',...(process.env.POOLSTER_RUNTIME_OFFLINE==='1'?['--offline']:[]),'--example','runtime_contract'],{cwd:sdk,env:{CARGO_TARGET_DIR:target},timeout:300000})
            return [join(target,'debug','examples','runtime_contract'),[],{}]
        }
        case 'swift':{
            const binary=join(root,'swift-probe');await exec('swiftc',['-parse-as-library',...await files(join(sdk,'Sources'),'.swift'),join(harness,'swift.swift'),'-o',binary],{cwd:sdk,timeout:180000,env:{CLANG_MODULE_CACHE_PATH:join(root,'clang-cache'),SWIFTPM_MODULECACHE_OVERRIDE:join(root,'swift-cache')}});return [binary,[],{}]
        }
        case 'typescript':{
            assert.ok(process.env.POOLSTER_TSC_JS,'TypeScript runner requires POOLSTER_TSC_JS')
            const compiled=join(root,'compiled');await exec('node',[process.env.POOLSTER_TSC_JS,'--strict','--target','ES2022','--module','commonjs','--moduleResolution','node','--lib','ES2022,DOM,DOM.Iterable','--skipLibCheck','--outDir',compiled,'index.ts'],{cwd:sdk})
            await writeFile(join(compiled,'package.json'),'{"type":"commonjs"}')
            return ['node',[join(harness,'typescript.cjs')],{POOLSTER_CONTRACT_COMPILED:compiled}]
        }
        case 'java':{
            const classpath=join(root,'java-classpath.txt')
            await exec('mvn',['--batch-mode','--no-transfer-progress','-q','-DskipTests','-Dmaven.compiler.source=17','-Dmaven.compiler.target=17','package','dependency:build-classpath',`-Dmdep.outputFile=${classpath}`],{cwd:sdk,timeout:300000})
            const javaClasspath=join(sdk,'target','classes')+delimiter+(await readFile(classpath,'utf8')).trim()
            await cp(join(harness,'java.java'),join(root,'RuntimeContract.java'))
            await exec('javac',['-cp',javaClasspath,'-d',root,join(root,'RuntimeContract.java')],{cwd:sdk})
            return ['java',['-cp',root+delimiter+javaClasspath,'RuntimeContract'],{}]
        }
        case 'csharp':{
            const probe=join(root,'probe');await mkdir(probe)
            const project=(await readdir(sdk)).find(file=>file.endsWith('.csproj'));assert.ok(project,'missing generated C# project')
            await writeFile(join(probe,'RuntimeContract.csproj'),`<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup><ItemGroup><ProjectReference Include="${join(sdk,project)}" /></ItemGroup></Project>`)
            await cp(join(harness,'csharp.cs'),join(probe,'Program.cs'))
            await exec('dotnet',['build','--configuration','Release','--nologo'],{cwd:probe,timeout:300000,env:{DOTNET_CLI_HOME:join(root,'dotnet-home')}})
            return ['dotnet',[join(probe,'bin','Release','net8.0','RuntimeContract.dll')],{}]
        }
        case 'php':{
            await exec('composer',['install','--no-interaction','--no-scripts','--no-plugins','--prefer-dist'],{cwd:sdk,timeout:300000})
            return ['php',[join(harness,'php.php')],{}]
        }
        case 'elixir':{
            await exec('mix',['deps.get'],{cwd:sdk,timeout:300000})
            await exec('mix',['compile','--warnings-as-errors'],{cwd:sdk,timeout:300000})
            return ['mix',['run',join(harness,'elixir.exs')],{}]
        }
        default:throw new Error(`No executable wire harness for ${language}; see coverage matrix`)
    }
}
export async function run(language,source){
    assert.ok(manifest.coverage[language],`unknown language ${language}`)
    assert.ok(manifest.coverage[language].supported.length,`No executable wire harness for ${language}`)
    const root=await mkdtemp(join(tmpdir(),'poolster-runtime-contract-'))
    let server
    try{
        const sdk=join(root,'sdk');await cp(resolve(source),sdk,{recursive:true})
        const [program,args,environment]=await prepare(language,sdk,root)
        server=await wireServer()
        for(const scenario of manifest.scenarios.filter(s=>manifest.coverage[language].supported.includes(s.id))){
            const stdout=await execute(program,args,{cwd:sdk,env:{...environment,POOLSTER_CONTRACT_URL:server.url,POOLSTER_CONTRACT_CASE:scenario.id,POOLSTER_CONTRACT_SCENARIO:JSON.stringify(scenario)},timeout:30000})
            const result=JSON.parse(stdout.trim().split('\n').at(-1));assert.equal(result.outcome,scenario.outcome,`${language}/${scenario.id}: outcome`)
            if(scenario.outcome==='success')assert.equal(result.id,scenario.contact_id,`${language}/${scenario.id}: model`)
            assert.equal(server.counts.get(scenario.id),scenario.requests,`${language}/${scenario.id}: HTTP attempt count`)
            validateKeyLifetime(scenario,server.requests.get(scenario.id)??[])
            assert.deepEqual(server.violations,[],`${language}/${scenario.id}: wire contract`)
            console.log(`PASS ${language}/${scenario.id}`)
        }
        for(const [scenario,reason] of Object.entries(manifest.coverage[language].unsupported??{}))console.log(`UNSUPPORTED ${language}/${scenario}: ${reason}`)
    }finally{if(server)await server.close();await rm(root,{recursive:true,force:true})}
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
    const [language,sdk]=process.argv.slice(2);assert.ok(language&&sdk,'usage: node runner.mjs LANGUAGE GENERATED_SDK_DIRECTORY');await run(language,sdk)
}
