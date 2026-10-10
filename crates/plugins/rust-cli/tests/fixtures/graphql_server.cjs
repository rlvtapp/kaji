const http = require('node:http');
const graphql = require(process.env.POOLSTER_GRAPHQL_JS);
if (graphql.version !== '16.14.2') throw new Error('Requires pinned GraphQL.js 16.14.2');
const schema = graphql.buildSchema(require('node:fs').readFileSync(process.argv[2], 'utf8'));
const server = http.createServer(async (request, response) => {
  let body = ''; for await (const chunk of request) body += chunk;
  if (request.headers.authorization !== 'Bearer secret' || request.headers['x-test'] !== 'configured') { response.writeHead(401); response.end(JSON.stringify({errors:[{message:'auth required'}]})); return; }
  const payload = JSON.parse(body);
  const result = await graphql.graphql({schema, source:payload.query, operationName:payload.operationName, variableValues:payload.variables, rootValue:{
    user:({id}) => ({id,name:'Ada',fragile:()=>{throw new Error('field failed')}}),
    rename:({name})=>({id:'7',name}),
    fatal:()=>{throw new Error('fatal')},
    presence:({value})=>value===undefined?'absent':value===null?'null':value,
  }});
  response.setHeader('content-type','application/json');
  if(request.url==='/bad') response.end(JSON.stringify({errors:[{wrong:'entry'}]}));
  else response.end(JSON.stringify(result));
});
server.listen(0,'127.0.0.1',()=>console.log(`http://127.0.0.1:${server.address().port}/graphql`));
