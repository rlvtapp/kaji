import test from 'node:test'
import assert from 'node:assert/strict'
import {manifest,validateManifest,wireServer,execute,prepare} from './runner.mjs'
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
            for(const expected of scenario.responses){
                const response=await fetch(server.url+manifest.operation.path,{headers:{authorization:`Bearer ${scenario.id}`,'x-contract-middleware':'yes'}})
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
    const root=await mkdtemp(join(tmpdir(),'kaji-java-plan-'));const sdk=join(root,'sdk');await mkdir(sdk)
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
