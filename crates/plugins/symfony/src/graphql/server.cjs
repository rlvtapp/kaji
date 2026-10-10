'use strict';
const {buildSchema, graphql}=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');
const http=require('node:http');
const schema=buildSchema('scalar DateTime\ntype User { id: ID!, name: String!, when: DateTime! }\ntype Query { user(id: ID!): User! }\ntype Mutation { rename(id: ID!, name: String!): User! }');
const root={user:({id})=>({id,name:'Ada',when:'2026-10-10'}),rename:({id,name})=>({id,name,when:'2026-10-10'})};
const server=http.createServer(async(req,res)=>{
  if(req.method!=='POST'||req.headers.authorization!=='Bearer secret'){res.writeHead(401);res.end();return;}
  let body='';for await(const chunk of req)body+=chunk;
  const data=JSON.parse(body);
  const result=await graphql({schema,source:data.query,operationName:data.operationName,variableValues:data.variables,rootValue:root});
  res.setHeader('Content-Type','application/graphql-response+json');res.end(JSON.stringify(result));
});
server.listen(0,'0.0.0.0',()=>console.log(server.address().port));
