import test from 'node:test'
import assert from 'node:assert/strict'
import {manifest,validateManifest,validateKeyLifetime,wireServer,execute,prepare} from './runner.mjs'
test('each runtime explicitly accounts for every wire scenario',()=>validateManifest(manifest))
test('coverage cannot omit or double-count a scenario',()=>{
    const missing=structuredClone(manifest);missing.coverage.go.supported.pop();assert.throws(()=>validateManifest(missing))
    const duplicate=structuredClone(manifest);duplicate.coverage.go.supported.push('success');assert.throws(()=>validateManifest(duplicate))
})
test('child failures cannot masquerade as SDK error outcomes',async()=>{
    assert.equal((await execute(process.execPath,['-e','console.log("ok")'])).trim(),'ok')
    await assert.rejects(execute(process.execPath,['-e','process.exit(7)']),/exited 7/)
})
test('mock serves the manifest responses and verifies native wire attempts',async()=>{
    const server=await wireServer()
    try{
        for(const scenario of manifest.scenarios){
            for(const [index,expected] of scenario.responses.entries()){
                const response=await fetch(server.url+(scenario.operation??manifest.operation).path+ (scenario.requests_wire?.[index]?.query?'?'+new URLSearchParams(Object.entries(scenario.requests_wire[index].query).flatMap(([key,value])=>(Array.isArray(value)?value:[value]).map(item=>[key,item]))):''),{method:(scenario.operation??manifest.operation).method,...(scenario.requests_wire?.[index]?.body?{body:JSON.stringify(scenario.requests_wire[index].body)}:{}),headers:{...(scenario.requests_wire?.[index]?.headers??{}),authorization:`Bearer ${scenario.id}`,'x-contract-middleware':'yes'}})
                assert.equal(response.status,expected.status);assert.equal(await response.text(),expected.body)
            }
            assert.equal(server.counts.get(scenario.id),scenario.requests)
        }
        assert.deepEqual(server.violations,[])
        await fetch(server.url+'/wrong',{headers:{authorization:'Bearer success'}})
        assert.equal(server.violations.length,2)
    }finally{await server.close()}
})

test('Java harness preparation copies source and preserves its classpath',async()=>{
    const {mkdtemp,mkdir,writeFile,readFile,rm}=await import('node:fs/promises')
    const {tmpdir}=await import('node:os');const {join}=await import('node:path')
    const root=await mkdtemp(join(tmpdir(),'poolster-java-plan-'));const sdk=join(root,'sdk');await mkdir(sdk)
    const commands=[]
    try{
        const result=await prepare('java',sdk,root,async(program,args)=>{
            commands.push(program)
            if(program==='mvn')await writeFile(join(root,'java-classpath.txt'),'/cache/jackson.jar')
            if(program==='javac')assert.match(await readFile(join(root,'RuntimeContract.java'),'utf8'),/class RuntimeContract/)
            return ''
        })
        assert.deepEqual(commands,['mvn','javac']);assert.equal(result[0],'java');assert.ok(result[1].includes('RuntimeContract'))
        assert.match(result[1][1],/jackson.jar/)
    }finally{await rm(root,{recursive:true,force:true})}
})


test('key lifetime distinguishes automatic retries, fresh invocations and caller overrides',()=>{
 const first='abcdef01-2345-4567-89ab-123456789abc',second='abcdef02-2345-4567-89ab-123456789abc'
 const captures=keys=>keys.map(key=>({headers:{'x-once':key}}))
 validateKeyLifetime({id:'retry',key_policy:'stable'},captures([first,first]))
 assert.throws(()=>validateKeyLifetime({id:'retry',key_policy:'stable'},captures([first,second])),/key lifetime/)
 validateKeyLifetime({id:'fresh',key_policy:'fresh'},captures([first,second]))
 assert.throws(()=>validateKeyLifetime({id:'fresh',key_policy:'fresh'},captures([first,first])),/key lifetime/)
 assert.throws(()=>validateKeyLifetime({id:'override',key_policy:'override',caller_key:'caller'},captures([first])),/caller key/)
 assert.throws(()=>validateKeyLifetime({id:'blank',key_policy:'blank'},captures([first])),/blank key/)
})

test('serialization wire checks reject lost falsy values, body null, header and array values',async()=>{
 const server=await wireServer()
 try{
  const scenario=manifest.scenarios.find(item=>item.id==='serialize_controls')
  await fetch(server.url+scenario.operation.path+'?text=wrong&flag=true&count=1&tags=a',{
   method:'POST',headers:{authorization:'Bearer serialize_controls','x-contract-middleware':'yes','x-label':'wrong'},body:JSON.stringify({enabled:true,count:1})})
  assert.deepEqual(server.violations,['serialize_controls: query mismatch','serialize_controls: header mismatch x-label','serialize_controls: body mismatch'])
 }finally{await server.close()}
})
