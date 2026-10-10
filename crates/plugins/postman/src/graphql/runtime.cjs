// Execute the emitted collection through pinned Postman/Newman, not a manual replay.
const fs = require('node:fs');
const http = require('node:http');
const assert = require('node:assert/strict');
const graphqlRoot = process.env.POOLSTER_GRAPHQL_JS_ROOT;
const newmanRoot = process.env.POOLSTER_NEWMAN_ROOT;
assert.equal(require(graphqlRoot + '/graphql/package.json').version, '16.14.2');
assert.equal(require(newmanRoot + '/newman/package.json').version, '6.2.1');
const { buildSchema, graphql } = require(graphqlRoot + '/graphql');
const newman = require(newmanRoot + '/newman');
const schema = buildSchema('type Query {user(id:ID!,show:Boolean,label:String):User!} type User {name:String! nickname:String} type Mutation {rename(name:String!):String!}');
const original = JSON.parse(fs.readFileSync(process.argv[2]));
const originalEnvironment = JSON.parse(fs.readFileSync(process.argv[3]));
let mode = 'success';
let calls = [];
const server = http.createServer(async (req, res) => {
  try {
    assert.equal(req.method, 'POST');
    assert.match(req.headers['content-type'], /^application\/json/);
    let body=''; for await(const chunk of req) body += chunk;
    const request = JSON.parse(body);
    calls.push(request.operationName);
    assert.ok(request.query.includes(request.operationName));
    if (request.operationName === 'ReadUser') {
      assert.deepEqual(request.variables, { id:'1', label:null });
      assert.equal(Object.hasOwn(request.variables, 'show'), false);
    } else assert.deepEqual(request.variables, { name:'example' });
    if (mode === 'protocol' && request.operationName === 'ReadUser') {
      res.setHeader('Content-Type','application/json'); res.end(JSON.stringify({unexpected:true})); return;
    }
    const result = await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:{
      user(args) {
        assert.equal(args.show,true); assert.equal(args.label,null);
        if(mode==='error') throw new Error('user failed');
        return {name:'Ada',nickname(){if(mode==='partial')throw new Error('nickname failed');return null;}};
      }, rename({name}) {return name;}
    }});
    res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));
  } catch(error) {res.statusCode=500;res.end(JSON.stringify({errors:[{message:error.message}]}));}
});
function run(collection,environment) {return new Promise((resolve,reject)=>newman.run({collection,environment,reporters:[],timeoutRequest:5000},(error,summary)=>error?reject(error):resolve(summary)));}
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 try {
  for(mode of ['success','partial','error','protocol']) {
   calls=[];const collection=structuredClone(original);const environment=structuredClone(originalEnvironment);
   environment.values.find(value=>value.key==='base_url').value=`http://127.0.0.1:${server.address().port}/graphql`;
   collection.event[0].script.exec.push(`pm.test('Recorded GraphQL status', () => pm.expect(pm.collectionVariables.get('poolster_graphql_status')).equal(pm.info.requestName === 'ReadUser' ? '${mode === 'protocol' ? 'protocol_error' : mode}' : 'success'));`);
   const summary=await run(collection,environment);
   assert.deepEqual(calls,['Rename','ReadUser']);
   const executions=summary.run.executions;
   assert.equal(executions.length,2);
   const response=executions[1].response.json();
   if(mode==='success'){assert.deepEqual(response.data,{user:{name:'Ada',nickname:null}});assert.equal(summary.run.failures.length,0);}
   if(mode==='partial'){assert.equal(response.data.user.name,'Ada');assert.equal(response.data.user.nickname,null);assert.equal(response.errors[0].message,'nickname failed');assert.deepEqual(response.errors[0].path,['user','nickname']);assert.equal(summary.run.failures.length,0);}
   if(mode==='error'){assert.equal(response.data,null);assert.equal(response.errors[0].message,'user failed');assert.equal(summary.run.failures.length,0);}
   if(mode==='protocol'){assert.equal(summary.run.failures.length,1);}

  }
  console.log('Newman GraphQL passed: success, partial, error, protocol and presence');
 } finally {await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);process.exitCode=1;});
